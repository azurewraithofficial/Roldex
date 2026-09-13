assert(plugin, "Roldex Studio Runtime must run as a Roblox Studio plugin")

local HttpService = game:GetService("HttpService")
local StudioCaptureService = game:GetService("StudioCaptureService")
local StudioTestService = game:GetService("StudioTestService")
local StudioDeviceSimulatorService = game:GetService("StudioDeviceSimulatorService")
local UserInputService = game:GetService("UserInputService")
local EncodingService = game:GetService("EncodingService")
local LogService = game:GetService("LogService")
local ReflectionService = game:GetService("ReflectionService")
local ScriptEditorService = game:GetService("ScriptEditorService")
local StarterPlayer = game:GetService("StarterPlayer")

local DEFAULT_BRIDGE_URL = "http://127.0.0.1:38247"
local MAX_LOGS = 200
local MAX_STEPS = 80
local MAX_CAPTURES = 2
local MAX_CAPTURE_WIDTH = 960
local MAX_CAPTURE_HEIGHT = 540

local CHAT_HARNESS_SOURCE = [[
local TextChatService = game:GetService("TextChatService")
local StudioTestService = game:GetService("StudioTestService")

local args = StudioTestService:GetTestArgs()
if type(args) ~= "table" then
	return
end

local messages = args.__roldex_chat_messages
if type(messages) ~= "table" or #messages == 0 then
	return
end

local channels = TextChatService:WaitForChild("TextChannels", 10)
if not channels then
	warn("[RoldexChatTest][ERROR] TextChatService.TextChannels was not created")
	return
end

local started = os.clock()
for _, entry in ipairs(messages) do
	local targetAt = math.max(tonumber(entry.at_seconds) or 0, 0)
	local remaining = targetAt - (os.clock() - started)
	if remaining > 0 then
		task.wait(remaining)
	end

	local channelName = tostring(entry.channel or "RBXGeneral")
	local channel = channels:FindFirstChild(channelName)
	if not channel then
		channel = channels:WaitForChild(channelName, 5)
	end
	if not channel and channelName ~= "RBXGeneral" then
		channel = channels:FindFirstChild("RBXGeneral") or channels:WaitForChild("RBXGeneral", 5)
	end
	if not channel or not channel:IsA("TextChannel") then
		warn("[RoldexChatTest][ERROR] Chat channel was unavailable: " .. channelName)
		continue
	end

	local text = tostring(entry.text or "Roldex chat test")
	local metadata = tostring(entry.metadata or "")
	local ok, result = pcall(function()
		return channel:SendAsync(text, metadata)
	end)
	if ok then
		print(("[RoldexChatTest][SENT] %s :: %s"):format(channel.Name, text))
	else
		warn("[RoldexChatTest][ERROR] " .. tostring(result))
	end
end
]]

local function sanitizeBridgeUrl(value)
	value = tostring(value or ""):gsub("%s+$", ""):gsub("^%s+", ""):gsub("/$", "")
	if value:match("^http://127%.0%.0%.1:%d+$") or value:match("^http://localhost:%d+$") then
		return value
	end
	return DEFAULT_BRIDGE_URL
end

local function bridgeUrl()
	return sanitizeBridgeUrl(plugin:GetSetting("RoldexBridgeUrl") or DEFAULT_BRIDGE_URL)
end

local function request(options)
	options.Headers = options.Headers or {}
	options.Headers["X-Roldex-Bridge"] = "studio"
	return HttpService:RequestAsync(options)
end

local function shortValue(value, maxChars)
	local text = tostring(value)
	if #text > maxChars then
		return text:sub(1, maxChars) .. "…"
	end
	return text
end

local function encodeBufferToBase64(value)
	local encoded = EncodingService:Base64Encode(value)
	return buffer.tostring(encoded)
end

local function captureScreenshot(payload)
	if not StudioCaptureService:CanCaptureScreenshot() then
		local granted = StudioCaptureService:RequestScreenshotPermissionAsync()
		assert(granted, "Studio screenshot permission was not granted")
	end

	local width = math.clamp(tonumber(payload.width) or 960, 160, MAX_CAPTURE_WIDTH)
	local height = math.clamp(tonumber(payload.height) or 540, 90, MAX_CAPTURE_HEIGHT)
	local includeUi = payload.include_ui ~= false
	local capture = StudioCaptureService:CaptureScreenshot({
		BufferFormat = Enum.StudioCaptureScreenshotFormat.PNG,
		Resolution = Vector2.new(width, height),
		UICaptureMode = includeUi and Enum.UICaptureMode.All or Enum.UICaptureMode.None,
	})
	assert(capture, "StudioCaptureService did not return a screenshot capture")

	local started = os.clock()
	while capture.BufferStatus ~= Enum.StudioCaptureBufferStatus.Ready do
		if capture.BufferStatus == Enum.StudioCaptureBufferStatus.Error then
			local errors = capture:GetErrors()
			error("Studio screenshot capture failed: " .. shortValue(HttpService:JSONEncode(errors), 2000))
		end
		if os.clock() - started > 8 then
			error("Studio screenshot capture timed out")
		end
		task.wait(0.05)
	end

	local png = capture:GetBuffer()
	return {
		data_base64 = encodeBufferToBase64(png),
		width = capture.Resolution.X,
		height = capture.Resolution.Y,
		include_ui = includeUi,
		format = tostring(capture.BufferFormat),
	}
end

local function virtualInput()
	local input = UserInputService:CreateVirtualInput()
	assert(input, "VirtualInput is unavailable in this Studio session")
	return input
end

local function enumItem(enumName, itemName)
	local enumType = Enum[enumName]
	assert(enumType, "unknown enum " .. tostring(enumName))
	local item = enumType[itemName]
	assert(item, "unknown enum item " .. tostring(enumName) .. "." .. tostring(itemName))
	return item
end

local function chatStepDelay(step)
	return math.clamp(tonumber(step.delay_seconds) or 0, 0, 10)
end

local function chatStepSettle(step)
	return math.clamp(tonumber(step.settle_seconds) or 0.9, 0.1, 5)
end

local function estimatedStepDuration(step)
	local kind = tostring(step.kind or "wait")
	if kind == "wait" then
		return math.clamp(tonumber(step.seconds) or 0.1, 0, 10)
	elseif kind == "key" or kind == "mouse_button" then
		return math.clamp(tonumber(step.hold_seconds) or 0.05, 0, 5)
	elseif kind == "chat" then
		return chatStepDelay(step) + chatStepSettle(step)
	elseif kind == "capture" then
		return 0.15
	end
	return 0
end

local function buildChatSchedule(steps, startDelay)
	local elapsed = math.max(tonumber(startDelay) or 0, 0)
	local messages = {}
	for index, step in ipairs(steps or {}) do
		if index > MAX_STEPS then
			break
		end
		local kind = tostring(step.kind or "wait")
		if kind == "chat" then
			local sendAt = elapsed + chatStepDelay(step)
			table.insert(messages, {
				at_seconds = sendAt,
				text = tostring(step.text or "Roldex chat test"),
				metadata = tostring(step.metadata or ""),
				channel = tostring(step.channel or "RBXGeneral"),
			})
		end
		elapsed += estimatedStepDuration(step)
	end
	return messages
end

local function installChatHarness()
	local starterPlayerScripts = StarterPlayer:WaitForChild("StarterPlayerScripts")
	local harness = Instance.new("LocalScript")
	harness.Name = "__RoldexChatTestHarness_" .. HttpService:GenerateGUID(false):gsub("%-", "")
	harness.Archivable = false
	harness.Parent = starterPlayerScripts
	local ok, errorMessage = pcall(function()
		ScriptEditorService:UpdateSourceAsync(harness, function()
			return CHAT_HARNESS_SOURCE
		end)
	end)
	if not ok then
		harness:Destroy()
		error("could not install temporary Roldex chat test harness: " .. tostring(errorMessage))
	end
	return harness
end

local function runInputSteps(steps, captures)
	local input = virtualInput()
	for index, step in ipairs(steps or {}) do
		if index > MAX_STEPS then
			break
		end
		local kind = tostring(step.kind or "wait")
		if kind == "wait" then
			task.wait(math.clamp(tonumber(step.seconds) or 0.1, 0, 10))
		elseif kind == "key" then
			local key = enumItem("KeyCode", tostring(step.key))
			local hold = math.clamp(tonumber(step.hold_seconds) or 0.05, 0, 5)
			input:SendKey(true, key, false)
			if hold > 0 then
				task.wait(hold)
			end
			input:SendKey(false, key, false)
		elseif kind == "mouse_move" then
			input:SendMousePosition(Vector2.new(tonumber(step.x) or 0, tonumber(step.y) or 0))
		elseif kind == "mouse_delta" then
			input:SendMouseDelta(Vector2.new(tonumber(step.x) or 0, tonumber(step.y) or 0))
		elseif kind == "mouse_button" then
			local button = enumItem("UserInputType", tostring(step.button or "MouseButton1"))
			local position = Vector2.new(tonumber(step.x) or 0, tonumber(step.y) or 0)
			input:SendMouseButton(position, button, true, tonumber(step.repeat_count) or 0)
			task.wait(math.clamp(tonumber(step.hold_seconds) or 0.05, 0, 5))
			input:SendMouseButton(position, button, false, tonumber(step.repeat_count) or 0)
		elseif kind == "text" then
			input:SendTextInput(tostring(step.text or ""))
		elseif kind == "chat" then
			task.wait(chatStepDelay(step) + chatStepSettle(step))
		elseif kind == "pointer" then
			local action = {}
			if step.wheel then
				action.Wheel = tonumber(step.wheel) or 0
			end
			if step.pan_x or step.pan_y then
				action.Pan = Vector2.new(tonumber(step.pan_x) or 0, tonumber(step.pan_y) or 0)
			end
			if step.pinch then
				action.Pinch = tonumber(step.pinch) or 0
			end
			input:SendPointerAction(Vector2.new(tonumber(step.x) or 0, tonumber(step.y) or 0), action)
		elseif kind == "capture" then
			if #captures < MAX_CAPTURES then
				table.insert(captures, captureScreenshot(step))
			end
		else
			error("unsupported virtual-input step kind " .. kind)
		end
	end
end

local function collectLogs()
	local logs = {}
	local warningCount = 0
	local errorCount = 0
	local connection = LogService.MessageOut:Connect(function(message, messageType)
		if #logs < MAX_LOGS then
			table.insert(logs, {
				message = shortValue(message, 1800),
				message_type = tostring(messageType),
			})
		end
		if messageType == Enum.MessageType.MessageWarning then
			warningCount += 1
		elseif messageType == Enum.MessageType.MessageError then
			errorCount += 1
		end
	end)
	return logs, function()
		connection:Disconnect()
		return warningCount, errorCount
	end
end

local function copyDictionary(value)
	local result = {}
	for key, item in pairs(value or {}) do
		result[key] = item
	end
	return result
end

local function runScenarioTest(payload)
	local mode = tostring(payload.mode or "play")
	local players = math.clamp(tonumber(payload.players) or 2, 1, 8)
	local timeoutSeconds = math.clamp(tonumber(payload.timeout_seconds) or 15, 2, 60)
	local startDelay = math.clamp(tonumber(payload.start_delay_seconds) or 1.0, 0, 10)
	local chatMessages = buildChatSchedule(payload.steps or {}, startDelay)
	if #chatMessages > 0 then
		assert(mode ~= "run", "chat scenario steps require play or multiplayer mode because TextChannel:SendAsync is client-side")
		assert(payload.args == nil or type(payload.args) == "table", "chat scenario steps require table-shaped test args")
	end

	local captures = {}
	local logs, finishLogs = collectLogs()
	local scenarioError = nil
	local chatHarness = nil
	local args = payload.args
	if args == nil then
		args = { roldex = true, visual_test = true }
	end
	if #chatMessages > 0 then
		args = copyDictionary(args)
		args.__roldex_chat_messages = chatMessages
		chatHarness = installChatHarness()
	end

	local runner = task.spawn(function()
		local ok, errorMessage = pcall(function()
			task.wait(startDelay)
			runInputSteps(payload.steps or {}, captures)
			if payload.capture_at_end ~= false and #captures < MAX_CAPTURES then
				task.wait(0.15)
				table.insert(captures, captureScreenshot({
					width = payload.capture_width,
					height = payload.capture_height,
					include_ui = payload.include_ui,
				}))
			end
		end)
		if not ok then
			scenarioError = tostring(errorMessage)
		end
		pcall(function()
			if StudioTestService:CanLeaveTest() then
				StudioTestService:LeaveTest()
			end
		end)
	end)

	local timeoutTask = task.delay(timeoutSeconds, function()
		pcall(function()
			if StudioTestService:CanLeaveTest() then
				StudioTestService:LeaveTest()
			end
		end)
	end)

	local ok, result = pcall(function()
		if mode == "run" then
			return StudioTestService:ExecuteRunModeAsync(args)
		elseif mode == "play" then
			return StudioTestService:ExecutePlayModeAsync(args)
		elseif mode == "multiplayer" then
			return StudioTestService:ExecuteMultiplayerTestAsync(players, args)
		end
		error("unsupported Studio scenario-test mode " .. mode)
	end)

	pcall(function()
		task.cancel(timeoutTask)
	end)
	pcall(function()
		task.cancel(runner)
	end)
	if chatHarness then
		pcall(function()
			chatHarness:Destroy()
		end)
	end
	local warningCount, errorCount = finishLogs()
	if not ok then
		error(result)
	end
	if scenarioError then
		error("input/capture scenario failed: " .. scenarioError)
	end

	return {
		mode = mode,
		players = mode == "multiplayer" and players or nil,
		result = tostring(result),
		test_goal = tostring(payload.review_goal or ""),
		chat_messages_requested = #chatMessages,
		warning_count = warningCount,
		error_count = errorCount,
		logs = logs,
		captures = captures,
	}
end

local function runInput(payload)
	local captures = {}
	runInputSteps(payload.steps or {}, captures)
	return { steps_completed = math.min(#(payload.steps or {}), MAX_STEPS), captures = captures }
end

local function deviceStatus()
	local device = StudioDeviceSimulatorService:GetDeviceAsync()
	local result = { device_id = device }
	if device ~= "default" then
		result.resolution = tostring(StudioDeviceSimulatorService:GetResolutionAsync())
		result.orientation = tostring(StudioDeviceSimulatorService:GetOrientationAsync())
		result.pixel_density = StudioDeviceSimulatorService:GetPixelDensityAsync()
		result.scaling_mode = tostring(StudioDeviceSimulatorService:GetScalingModeAsync())
	end
	return result
end

local function runDevice(payload)
	local operation = tostring(payload.operation or "status")
	if operation == "status" then
		return deviceStatus()
	elseif operation == "list" then
		local ids = StudioDeviceSimulatorService:GetDeviceListAsync()
		local devices = {}
		for _, id in ipairs(ids) do
			local ok, info = pcall(function()
				return StudioDeviceSimulatorService:GetDeviceInfoAsync(id)
			end)
			table.insert(devices, ok and info or { DeviceId = id })
		end
		return { devices = devices }
	elseif operation == "set_device" then
		assert(type(payload.device_id) == "string" and payload.device_id ~= "", "set_device requires device_id")
		StudioDeviceSimulatorService:SetDeviceAsync(payload.device_id)
		return deviceStatus()
	elseif operation == "set_resolution" then
		StudioDeviceSimulatorService:SetResolutionAsync(
			math.clamp(tonumber(payload.width) or 1280, 1, 7680),
			math.clamp(tonumber(payload.height) or 720, 1, 4320)
		)
		return deviceStatus()
	elseif operation == "set_orientation" then
		local orientation = tostring(payload.orientation or "landscape")
		local value = if orientation == "portrait"
			then Enum.ScreenOrientation.Portrait
			elseif orientation == "landscape_right"
			then Enum.ScreenOrientation.LandscapeRight
			else Enum.ScreenOrientation.LandscapeLeft
		StudioDeviceSimulatorService:SetOrientationAsync(value)
		return deviceStatus()
	elseif operation == "set_dpi" then
		StudioDeviceSimulatorService:SetPixelDensityAsync(math.clamp(tonumber(payload.dpi) or 160, 72, 10000))
		return deviceStatus()
	elseif operation == "set_scaling" then
		local mode = tostring(payload.scaling_mode or "fit")
		local value = if mode == "actual"
			then Enum.DeviceSimulatorScalingMode.ActualResolution
			elseif mode == "physical"
			then Enum.DeviceSimulatorScalingMode.ScaleToPhysicalSize
			else Enum.DeviceSimulatorScalingMode.FitToWindow
		StudioDeviceSimulatorService:SetScalingModeAsync(value)
		return deviceStatus()
	elseif operation == "stop" then
		StudioDeviceSimulatorService:StopSimulationAsync()
		return deviceStatus()
	end
	error("unsupported device operation " .. operation)
end

local function reflectApi(payload)
	local className = tostring(payload.class_name or "")
	assert(className ~= "", "reflect requires class_name")
	local filter = payload.filter or {}
	local result = { class_name = className }
	if payload.include_properties ~= false then
		local properties = ReflectionService:GetPropertiesOfClass(className, filter)
		result.properties = {}
		for index, item in ipairs(properties) do
			if index > 120 then break end
			table.insert(result.properties, tostring(item))
		end
	end
	if payload.include_methods ~= false then
		local methods = ReflectionService:GetMethodsOfClass(className, filter)
		result.methods = {}
		for index, item in ipairs(methods) do
			if index > 120 then break end
			table.insert(result.methods, tostring(item))
		end
	end
	if payload.include_events then
		local events = ReflectionService:GetEventsOfClass(className, filter)
		result.events = {}
		for index, item in ipairs(events) do
			if index > 120 then break end
			table.insert(result.events, tostring(item))
		end
	end
	return result
end

local function executeCommand(command)
	local ok, output = pcall(function()
		if command.action == "capture" then
			return captureScreenshot(command.payload or {})
		elseif command.action == "scenario_test" then
			return runScenarioTest(command.payload or {})
		elseif command.action == "input" then
			return runInput(command.payload or {})
		elseif command.action == "device" then
			return runDevice(command.payload or {})
		elseif command.action == "reflect" then
			return reflectApi(command.payload or {})
		end
		error("unsupported Roldex Studio Runtime command " .. tostring(command.action))
	end)
	if ok then
		return { id = command.id, ok = true, output = output }
	end
	return { id = command.id, ok = false, error = tostring(output) }
end

local function postResult(result)
	request({
		Url = bridgeUrl() .. "/v1/actions/result",
		Method = "POST",
		Headers = { ["Content-Type"] = "application/json" },
		Body = HttpService:JSONEncode(result),
	})
end

local function poll()
	local response = request({ Url = bridgeUrl() .. "/v1/runtime-actions", Method = "GET" })
	if not response.Success then
		return
	end
	local decoded = HttpService:JSONDecode(response.Body)
	for _, command in ipairs(decoded.commands or {}) do
		local result = executeCommand(command)
		postResult(result)
	end
end

task.spawn(function()
	while true do
		task.wait(0.12)
		pcall(poll)
	end
end)