assert(plugin, "Roldex Studio Runtime must run as a Roblox Studio plugin")

local HttpService = game:GetService("HttpService")
local StudioCaptureService = game:GetService("StudioCaptureService")
local StudioTestService = game:GetService("StudioTestService")
local UserInputService = game:GetService("UserInputService")
local EncodingService = game:GetService("EncodingService")
local LogService = game:GetService("LogService")
local ReflectionService = game:GetService("ReflectionService")

local DEFAULT_BRIDGE_URL = "http://127.0.0.1:38247"
local MAX_LOGS = 200
local MAX_STEPS = 80
local MAX_CAPTURES = 2
local MAX_CAPTURE_WIDTH = 960
local MAX_CAPTURE_HEIGHT = 540

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
		png_base64 = encodeBufferToBase64(png),
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

local function runScenarioTest(payload)
	local mode = tostring(payload.mode or "play")
	local players = math.clamp(tonumber(payload.players) or 2, 1, 8)
	local timeoutSeconds = math.clamp(tonumber(payload.timeout_seconds) or 15, 2, 60)
	local captures = {}
	local logs, finishLogs = collectLogs()
	local scenarioError = nil

	local runner = task.spawn(function()
		local ok, errorMessage = pcall(function()
			task.wait(math.clamp(tonumber(payload.start_delay_seconds) or 1.0, 0, 10))
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
		local args = payload.args or { roldex = true, visual_test = true }
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
