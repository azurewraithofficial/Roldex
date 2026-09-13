assert(plugin, "Roldex Studio must run as a Roblox Studio plugin")

local HttpService = game:GetService("HttpService")
local Selection = game:GetService("Selection")
local ScriptEditorService = game:GetService("ScriptEditorService")
local StudioService = game:GetService("StudioService")
local ChangeHistoryService = game:GetService("ChangeHistoryService")
local CollectionService = game:GetService("CollectionService")
local StudioTestService = game:GetService("StudioTestService")
local StudioCaptureService = game:GetService("StudioCaptureService")
local StudioDeviceSimulatorService = game:GetService("StudioDeviceSimulatorService")
local EncodingService = game:GetService("EncodingService")
local UserInputService = game:GetService("UserInputService")
local LogService = game:GetService("LogService")

local DEFAULT_BRIDGE_URL = "http://127.0.0.1:38247"
local MAX_SELECTION_ITEMS = 24
local MAX_SOURCE_CHARS = 80000
local MAX_ATTRIBUTES = 24
local MAX_DETAIL_CHARS = 3200
local MAX_QUERY_RESULTS = 100
local MAX_LOGS = 250
local MAX_DEVICE_RESULTS = 100

local colors = {
	background = Color3.fromRGB(20, 22, 27),
	panel = Color3.fromRGB(29, 32, 39),
	panelAlt = Color3.fromRGB(35, 39, 47),
	text = Color3.fromRGB(239, 242, 247),
	muted = Color3.fromRGB(161, 169, 184),
	accent = Color3.fromRGB(74, 222, 128),
	accentDark = Color3.fromRGB(36, 130, 77),
	danger = Color3.fromRGB(248, 113, 113),
}

local toolbar = plugin:CreateToolbar("Roldex")
local toolbarButton = toolbar:CreateButton(
	"RoldexOpen",
	"Open the Roldex Roblox development agent",
	"",
	"Roldex"
)
toolbarButton.ClickableWhenViewportHidden = true

local widgetInfo = DockWidgetPluginGuiInfo.new(
	Enum.InitialDockState.Right,
	false,
	false,
	460,
	680,
	360,
	480
)
local widget = plugin:CreateDockWidgetPluginGuiAsync("RoldexStudioDock", widgetInfo)
widget.Title = "Roldex Studio"

local root = Instance.new("Frame")
root.Name = "Root"
root.Size = UDim2.fromScale(1, 1)
root.BackgroundColor3 = colors.background
root.BorderSizePixel = 0
root.Parent = widget

local rootPadding = Instance.new("UIPadding")
rootPadding.PaddingTop = UDim.new(0, 12)
rootPadding.PaddingBottom = UDim.new(0, 12)
rootPadding.PaddingLeft = UDim.new(0, 12)
rootPadding.PaddingRight = UDim.new(0, 12)
rootPadding.Parent = root

local function makeLabel(name, text, position, size, textSize, color)
	local object = Instance.new("TextLabel")
	object.Name = name
	object.BackgroundTransparency = 1
	object.Position = position
	object.Size = size
	object.Font = Enum.Font.Gotham
	object.Text = text
	object.TextSize = textSize
	object.TextColor3 = color or colors.text
	object.TextXAlignment = Enum.TextXAlignment.Left
	object.TextYAlignment = Enum.TextYAlignment.Center
	object.Parent = root
	return object
end

local function makeButton(name, text, position, size)
	local object = Instance.new("TextButton")
	object.Name = name
	object.Position = position
	object.Size = size
	object.BackgroundColor3 = colors.panelAlt
	object.BorderSizePixel = 0
	object.Active = true
	object.AutoButtonColor = true
	object.Font = Enum.Font.GothamSemibold
	object.Text = text
	object.TextSize = 13
	object.TextColor3 = colors.text
	object.Parent = root
	local corner = Instance.new("UICorner")
	corner.CornerRadius = UDim.new(0, 6)
	corner.Parent = object
	return object
end

local title = makeLabel("Title", "ROLDEX", UDim2.new(0, 0, 0, 0), UDim2.new(1, 0, 0, 28), 20)
title.Font = Enum.Font.GothamBold

local statusLabel = makeLabel(
	"Status",
	"● Checking bridge...",
	UDim2.new(0, 0, 0, 30),
	UDim2.new(1, 0, 0, 22),
	12,
	colors.muted
)

local selectionLabel = makeLabel(
	"Selection",
	"Selection: none",
	UDim2.new(0, 0, 0, 56),
	UDim2.new(1, 0, 0, 38),
	12,
	colors.muted
)
selectionLabel.TextWrapped = true
selectionLabel.TextYAlignment = Enum.TextYAlignment.Top

local bridgeUrlBox = Instance.new("TextBox")
bridgeUrlBox.Name = "BridgeUrl"
bridgeUrlBox.Position = UDim2.new(0, 0, 0, 100)
bridgeUrlBox.Size = UDim2.new(1, 0, 0, 30)
bridgeUrlBox.BackgroundColor3 = colors.panel
bridgeUrlBox.BorderSizePixel = 0
bridgeUrlBox.ClearTextOnFocus = false
bridgeUrlBox.Font = Enum.Font.Code
bridgeUrlBox.PlaceholderText = DEFAULT_BRIDGE_URL
bridgeUrlBox.Text = plugin:GetSetting("RoldexBridgeUrl") or DEFAULT_BRIDGE_URL
bridgeUrlBox.TextColor3 = colors.text
bridgeUrlBox.PlaceholderColor3 = colors.muted
bridgeUrlBox.TextSize = 12
bridgeUrlBox.TextXAlignment = Enum.TextXAlignment.Left
bridgeUrlBox.Parent = root
local urlPadding = Instance.new("UIPadding")
urlPadding.PaddingLeft = UDim.new(0, 8)
urlPadding.PaddingRight = UDim.new(0, 8)
urlPadding.Parent = bridgeUrlBox
local urlCorner = Instance.new("UICorner")
urlCorner.CornerRadius = UDim.new(0, 6)
urlCorner.Parent = bridgeUrlBox

local promptBox = Instance.new("TextBox")
promptBox.Name = "Prompt"
promptBox.Position = UDim2.new(0, 0, 0, 140)
promptBox.Size = UDim2.new(1, 0, 0, 124)
promptBox.BackgroundColor3 = colors.panel
promptBox.BorderSizePixel = 0
promptBox.ClearTextOnFocus = false
promptBox.Font = Enum.Font.Code
promptBox.MultiLine = true
promptBox.PlaceholderText = "Describe the finished Roblox result. Roldex can research, build live, script, test, inspect visually, repair, and verify it..."
promptBox.Text = ""
promptBox.TextColor3 = colors.text
promptBox.PlaceholderColor3 = colors.muted
promptBox.TextSize = 13
promptBox.TextWrapped = true
promptBox.TextXAlignment = Enum.TextXAlignment.Left
promptBox.TextYAlignment = Enum.TextYAlignment.Top
promptBox.Parent = root
local promptPadding = Instance.new("UIPadding")
promptPadding.PaddingTop = UDim.new(0, 8)
promptPadding.PaddingBottom = UDim.new(0, 8)
promptPadding.PaddingLeft = UDim.new(0, 8)
promptPadding.PaddingRight = UDim.new(0, 8)
promptPadding.Parent = promptBox
local promptCorner = Instance.new("UICorner")
promptCorner.CornerRadius = UDim.new(0, 6)
promptCorner.Parent = promptBox

local sendButton = makeButton(
	"Send",
	"Build / Fix / Test",
	UDim2.new(0, 0, 0, 274),
	UDim2.new(0.68, -4, 0, 34)
)
sendButton.BackgroundColor3 = colors.accentDark
local healthButton = makeButton(
	"Reconnect",
	"Reconnect",
	UDim2.new(0.68, 4, 0, 274),
	UDim2.new(0.32, -4, 0, 34)
)

local outputFrame = Instance.new("ScrollingFrame")
outputFrame.Name = "Output"
outputFrame.Position = UDim2.new(0, 0, 0, 318)
outputFrame.Size = UDim2.new(1, 0, 1, -318)
outputFrame.BackgroundColor3 = colors.panel
outputFrame.BorderSizePixel = 0
outputFrame.AutomaticCanvasSize = Enum.AutomaticSize.Y
outputFrame.CanvasSize = UDim2.new()
outputFrame.ScrollBarThickness = 6
outputFrame.Parent = root
local outputCorner = Instance.new("UICorner")
outputCorner.CornerRadius = UDim.new(0, 6)
outputCorner.Parent = outputFrame

local outputLabel = Instance.new("TextLabel")
outputLabel.Name = "Answer"
outputLabel.Size = UDim2.new(1, -16, 0, 0)
outputLabel.Position = UDim2.new(0, 8, 0, 8)
outputLabel.AutomaticSize = Enum.AutomaticSize.Y
outputLabel.BackgroundTransparency = 1
outputLabel.Font = Enum.Font.Code
outputLabel.Text = "Roldex is ready. Keep Studio open and watch the place change while it works."
outputLabel.TextColor3 = colors.text
outputLabel.TextSize = 13
outputLabel.TextWrapped = true
outputLabel.TextXAlignment = Enum.TextXAlignment.Left
outputLabel.TextYAlignment = Enum.TextYAlignment.Top
outputLabel.Parent = outputFrame

local pollBusy = false
local pluginConnected = false

local function setStatus(text, kind)
	statusLabel.Text = "● " .. text
	if kind == "ok" then
		statusLabel.TextColor3 = colors.accent
	elseif kind == "error" then
		statusLabel.TextColor3 = colors.danger
	else
		statusLabel.TextColor3 = colors.muted
	end
end

local function setButtonEnabled(button, enabled)
	button.Active = enabled
	button.AutoButtonColor = enabled
	button.TextColor3 = enabled and colors.text or colors.muted
end

local function sanitizeBridgeUrl(value)
	value = tostring(value or ""):gsub("%s+$", ""):gsub("^%s+", ""):gsub("/$", "")
	if value:match("^http://127%.0%.0%.1:%d+$") or value:match("^http://localhost:%d+$") then
		return value
	end
	return DEFAULT_BRIDGE_URL
end

local function bridgeUrl()
	local value = sanitizeBridgeUrl(bridgeUrlBox.Text)
	if bridgeUrlBox.Text ~= value then
		bridgeUrlBox.Text = value
	end
	return value
end

local function request(options)
	options.Headers = options.Headers or {}
	options.Headers["X-Roldex-Bridge"] = "studio"
	return HttpService:RequestAsync(options)
end

local function responseDetail(response)
	if type(response) ~= "table" then
		return tostring(response)
	end
	if response.Body and response.Body ~= "" then
		return ("HTTP %s: %s"):format(tostring(response.StatusCode), tostring(response.Body))
	end
	return "HTTP " .. tostring(response.StatusCode)
end

local function isScriptContainer(instance)
	return instance
		and (instance:IsA("Script") or instance:IsA("LocalScript") or instance:IsA("ModuleScript"))
end

local function shortValue(value, maxChars)
	local text = tostring(value)
	if #text > maxChars then
		return text:sub(1, maxChars) .. "…"
	end
	return text
end

local function encodeValue(value)
	local valueType = typeof(value)
	if valueType == "nil" or valueType == "boolean" or valueType == "number" or valueType == "string" then
		return value
	elseif valueType == "Vector3" then
		return { ["$type"] = "Vector3", x = value.X, y = value.Y, z = value.Z }
	elseif valueType == "Vector2" then
		return { ["$type"] = "Vector2", x = value.X, y = value.Y }
	elseif valueType == "Color3" then
		return { ["$type"] = "Color3", r = value.R, g = value.G, b = value.B }
	elseif valueType == "UDim" then
		return { ["$type"] = "UDim", scale = value.Scale, offset = value.Offset }
	elseif valueType == "UDim2" then
		return {
			["$type"] = "UDim2",
			x_scale = value.X.Scale,
			x_offset = value.X.Offset,
			y_scale = value.Y.Scale,
			y_offset = value.Y.Offset,
		}
	elseif valueType == "CFrame" then
		return { ["$type"] = "CFrame", components = { value:GetComponents() } }
	elseif valueType == "EnumItem" then
		return { ["$type"] = "Enum", enum_type = tostring(value.EnumType):gsub("^Enum%.", ""), item = value.Name }
	elseif valueType == "BrickColor" then
		return { ["$type"] = "BrickColor", name = value.Name }
	elseif valueType == "NumberRange" then
		return { ["$type"] = "NumberRange", min = value.Min, max = value.Max }
	elseif valueType == "Rect" then
		return {
			["$type"] = "Rect",
			min_x = value.Min.X,
			min_y = value.Min.Y,
			max_x = value.Max.X,
			max_y = value.Max.Y,
		}
	elseif valueType == "Instance" then
		return { ["$type"] = "Instance", path = value:GetFullName(), class_name = value.ClassName }
	end
	return tostring(value)
end

local function resolvePath(path, refs)
	if typeof(path) == "Instance" then
		return path
	end
	if type(path) ~= "string" or path == "" then
		return nil
	end
	if path == "game" then
		return game
	elseif path == "$activeScript" then
		return StudioService.ActiveScript
	end
	local selectionIndex = path:match("^%$selection:(%d+)$")
	if selectionIndex then
		return Selection:Get()[tonumber(selectionIndex)]
	end
	if path:sub(1, 1) == "$" and refs then
		return refs[path:sub(2)]
	end

	local original = path
	path = path:gsub("^game%.", "")
	local components = string.split(path, ".")
	if #components == 0 then
		return nil
	end

	local current
	local ok, service = pcall(function()
		return game:GetService(components[1])
	end)
	if ok then
		current = service
	else
		current = game:FindFirstChild(components[1])
	end
	for index = 2, #components do
		if not current then
			break
		end
		current = current:FindFirstChild(components[index])
	end
	if current then
		return current
	end

	for _, descendant in ipairs(game:GetDescendants()) do
		local fullName = descendant:GetFullName()
		if fullName == original or fullName == path then
			return descendant
		end
	end
	return nil
end

local function decodeValue(value, refs)
	if type(value) ~= "table" then
		return value
	end
	local typeName = value["$type"]
	if not typeName then
		return value
	end
	if typeName == "Vector3" then
		return Vector3.new(value.x or 0, value.y or 0, value.z or 0)
	elseif typeName == "Vector2" then
		return Vector2.new(value.x or 0, value.y or 0)
	elseif typeName == "Color3" then
		return Color3.new(value.r or 0, value.g or 0, value.b or 0)
	elseif typeName == "UDim" then
		return UDim.new(value.scale or 0, value.offset or 0)
	elseif typeName == "UDim2" then
		return UDim2.new(value.x_scale or 0, value.x_offset or 0, value.y_scale or 0, value.y_offset or 0)
	elseif typeName == "CFrame" then
		return CFrame.new(table.unpack(value.components or {}))
	elseif typeName == "Enum" then
		local enumType = Enum[value.enum_type]
		assert(enumType, "unknown Enum type " .. tostring(value.enum_type))
		local item = enumType[value.item]
		assert(item, "unknown Enum item " .. tostring(value.item))
		return item
	elseif typeName == "BrickColor" then
		return BrickColor.new(value.name)
	elseif typeName == "NumberRange" then
		return NumberRange.new(value.min or 0, value.max or value.min or 0)
	elseif typeName == "Rect" then
		return Rect.new(value.min_x or 0, value.min_y or 0, value.max_x or 0, value.max_y or 0)
	elseif typeName == "Instance" then
		local instance = resolvePath(value.path, refs)
		assert(instance, "could not resolve Instance path " .. tostring(value.path))
		return instance
	elseif typeName == "NumberSequence" then
		local keypoints = {}
		for _, keypoint in ipairs(value.keypoints or {}) do
			table.insert(keypoints, NumberSequenceKeypoint.new(keypoint.time, keypoint.value, keypoint.envelope or 0))
		end
		return NumberSequence.new(keypoints)
	elseif typeName == "ColorSequence" then
		local keypoints = {}
		for _, keypoint in ipairs(value.keypoints or {}) do
			table.insert(
				keypoints,
				ColorSequenceKeypoint.new(
					keypoint.time,
					Color3.new(keypoint.r or 0, keypoint.g or 0, keypoint.b or 0)
				)
			)
		end
		return ColorSequence.new(keypoints)
	end
	error("unsupported typed Studio value " .. tostring(typeName))
end

local function describeAttributes(instance, parts)
	local attributes = instance:GetAttributes()
	local names = {}
	for name in pairs(attributes) do
		table.insert(names, name)
	end
	table.sort(names)
	if #names == 0 then
		return
	end
	local formatted = {}
	for index, name in ipairs(names) do
		if index > MAX_ATTRIBUTES then
			table.insert(formatted, "…")
			break
		end
		table.insert(formatted, name .. "=" .. shortValue(attributes[name], 120))
	end
	table.insert(parts, "Attributes{" .. table.concat(formatted, ", ") .. "}")
end

local function describeInstance(instance)
	local parts = { "Children=" .. tostring(#instance:GetChildren()) }
	if instance:IsA("BasePart") then
		table.insert(parts, "Position=" .. shortValue(instance.Position, 120))
		table.insert(parts, "Size=" .. shortValue(instance.Size, 120))
		table.insert(parts, "Anchored=" .. tostring(instance.Anchored))
		table.insert(parts, "CanCollide=" .. tostring(instance.CanCollide))
		table.insert(parts, "CanQuery=" .. tostring(instance.CanQuery))
		table.insert(parts, "CanTouch=" .. tostring(instance.CanTouch))
		table.insert(parts, "Material=" .. tostring(instance.Material))
	elseif instance:IsA("Model") then
		local ok, pivot = pcall(function()
			return instance:GetPivot()
		end)
		if ok then
			table.insert(parts, "PivotPosition=" .. shortValue(pivot.Position, 120))
		end
		table.insert(parts, "PrimaryPart=" .. (instance.PrimaryPart and instance.PrimaryPart.Name or "none"))
	elseif instance:IsA("GuiObject") then
		table.insert(parts, "Position=" .. shortValue(instance.Position, 160))
		table.insert(parts, "Size=" .. shortValue(instance.Size, 160))
		table.insert(parts, "AbsolutePosition=" .. shortValue(instance.AbsolutePosition, 120))
		table.insert(parts, "AbsoluteSize=" .. shortValue(instance.AbsoluteSize, 120))
		table.insert(parts, "Visible=" .. tostring(instance.Visible))
		table.insert(parts, "ZIndex=" .. tostring(instance.ZIndex))
	elseif instance:IsA("Sound") then
		table.insert(parts, "SoundId=" .. shortValue(instance.SoundId, 240))
		table.insert(parts, "Volume=" .. tostring(instance.Volume))
		table.insert(parts, "Looped=" .. tostring(instance.Looped))
	elseif instance:IsA("ParticleEmitter") then
		table.insert(parts, "Enabled=" .. tostring(instance.Enabled))
		table.insert(parts, "Rate=" .. tostring(instance.Rate))
	end
	describeAttributes(instance, parts)
	local text = table.concat(parts, "; ")
	if #text > MAX_DETAIL_CHARS then
		text = text:sub(1, MAX_DETAIL_CHARS) .. "…"
	end
	return text
end

local function serializeInstance(instance, requestedProperties)
	local properties = {}
	for _, property in ipairs(requestedProperties or {}) do
		local ok, value = pcall(function()
			return instance[property]
		end)
		properties[property] = ok and encodeValue(value) or "<unreadable>"
	end
	local attributes = {}
	for name, value in pairs(instance:GetAttributes()) do
		attributes[name] = encodeValue(value)
	end
	return {
		name = instance.Name,
		class_name = instance.ClassName,
		full_name = instance:GetFullName(),
		children_count = #instance:GetChildren(),
		details = describeInstance(instance),
		attributes = attributes,
		properties = properties,
	}
end

local function collectSelection()
	local result = {}
	for index, instance in ipairs(Selection:Get()) do
		if index > MAX_SELECTION_ITEMS then
			break
		end
		table.insert(result, {
			name = instance.Name,
			class_name = instance.ClassName,
			full_name = instance:GetFullName(),
			details = describeInstance(instance),
		})
	end
	return result
end

local function collectActiveScript()
	local active = StudioService.ActiveScript
	if not isScriptContainer(active) then
		return nil
	end
	local ok, source = pcall(function()
		return ScriptEditorService:GetEditorSource(active)
	end)
	if not ok then
		source = ""
	end
	if #source > MAX_SOURCE_CHARS then
		source = source:sub(1, MAX_SOURCE_CHARS) .. "\n-- … source truncated by Roldex Studio"
	end
	return {
		class_name = active.ClassName,
		full_name = active:GetFullName(),
		source = source,
	}
end

local function updateSelectionLabel()
	local selected = Selection:Get()
	local active = StudioService.ActiveScript
	local parts = { ("Selection: %d"):format(#selected) }
	if isScriptContainer(active) then
		table.insert(parts, "Active: " .. active:GetFullName())
	end
	selectionLabel.Text = table.concat(parts, "  |  ")
end

local function setProperties(instance, properties, refs)
	for property, rawValue in pairs(properties or {}) do
		if property ~= "Parent" and property ~= "Source" then
			local value = decodeValue(rawValue, refs)
			local ok, errorMessage = pcall(function()
				instance[property] = value
			end)
			if not ok then
				error(("could not set %s.%s: %s"):format(instance:GetFullName(), property, tostring(errorMessage)))
			end
		end
	end
end

local function setAttributes(instance, attributes, refs)
	for name, rawValue in pairs(attributes or {}) do
		instance:SetAttribute(name, decodeValue(rawValue, refs))
	end
end

local function updateScriptSource(scriptObject, source)
	assert(isScriptContainer(scriptObject), "target is not a Script, LocalScript, or ModuleScript")
	assert(type(source) == "string", "script source must be a string")
	ScriptEditorService:UpdateSourceAsync(scriptObject, function()
		return source
	end)
end

local function executeQuery(payload)
	local operation = payload.operation
	if operation == "selection" then
		local items = {}
		for _, instance in ipairs(Selection:Get()) do
			if #items >= MAX_QUERY_RESULTS then
				break
			end
			table.insert(items, serializeInstance(instance, payload.properties))
		end
		return { items = items }
	elseif operation == "inspect" then
		local instance = resolvePath(payload.path)
		assert(instance, "could not resolve Studio path " .. tostring(payload.path))
		return { item = serializeInstance(instance, payload.properties) }
	elseif operation == "children" then
		local instance = resolvePath(payload.path)
		assert(instance, "could not resolve Studio path " .. tostring(payload.path))
		local items = {}
		for _, child in ipairs(instance:GetChildren()) do
			if #items >= math.clamp(tonumber(payload.max_results) or MAX_QUERY_RESULTS, 1, MAX_QUERY_RESULTS) then
				break
			end
			table.insert(items, serializeInstance(child, payload.properties))
		end
		return { items = items }
	elseif operation == "find" then
		local rootInstance = payload.root_path and resolvePath(payload.root_path) or game
		assert(rootInstance, "could not resolve find root")
		local maxResults = math.clamp(tonumber(payload.max_results) or 40, 1, MAX_QUERY_RESULTS)
		local nameContains = string.lower(tostring(payload.name_contains or ""))
		local className = payload.class_name
		local items = {}
		local candidates = rootInstance:GetDescendants()
		for _, instance in ipairs(candidates) do
			local nameMatches = nameContains == ""
				or string.find(string.lower(instance.Name), nameContains, 1, true) ~= nil
			local classMatches = not className or instance.ClassName == className or instance:IsA(className)
			if nameMatches and classMatches then
				table.insert(items, serializeInstance(instance, payload.properties))
				if #items >= maxResults then
					break
				end
			end
		end
		return { items = items }
	end
	error("unsupported Studio query operation " .. tostring(operation))
end

local function executeMutationAction(action, refs)
	local op = action.op
	if op == "create" then
		assert(type(action.class_name) == "string", "create requires class_name")
		local parent = resolvePath(action.parent or "Workspace", refs)
		assert(parent, "create parent could not be resolved")
		local instance = Instance.new(action.class_name)
		if action.name then
			instance.Name = action.name
		end
		setProperties(instance, action.properties, refs)
		setAttributes(instance, action.attributes, refs)
		instance.Parent = parent
		if action.source ~= nil then
			updateScriptSource(instance, action.source)
		end
		for _, tag in ipairs(action.tags or {}) do
			CollectionService:AddTag(instance, tag)
		end
		if action.ref then
			refs[action.ref] = instance
		end
		return instance
	elseif op == "set_properties" then
		local target = resolvePath(action.target or action.path, refs)
		assert(target, "set_properties target could not be resolved")
		setProperties(target, action.properties, refs)
		return target
	elseif op == "set_attributes" then
		local target = resolvePath(action.target or action.path, refs)
		assert(target, "set_attributes target could not be resolved")
		setAttributes(target, action.attributes, refs)
		for _, name in ipairs(action.remove_attributes or {}) do
			target:SetAttribute(name, nil)
		end
		return target
	elseif op == "move" then
		local target = resolvePath(action.target or action.path, refs)
		local parent = resolvePath(action.parent, refs)
		assert(target and parent, "move target/parent could not be resolved")
		target.Parent = parent
		return target
	elseif op == "clone" then
		local target = resolvePath(action.target or action.path, refs)
		assert(target, "clone target could not be resolved")
		local clone = target:Clone()
		if action.name then
			clone.Name = action.name
		end
		setProperties(clone, action.properties, refs)
		setAttributes(clone, action.attributes, refs)
		clone.Parent = resolvePath(action.parent, refs) or target.Parent
		if action.ref then
			refs[action.ref] = clone
		end
		return clone
	elseif op == "delete" then
		local target = resolvePath(action.target or action.path, refs)
		assert(target and target ~= game, "delete target could not be resolved or is forbidden")
		local fullName = target:GetFullName()
		target:Destroy()
		return fullName
	elseif op == "update_script" then
		local target = resolvePath(action.target or action.path, refs)
		assert(target, "update_script target could not be resolved")
		updateScriptSource(target, action.source)
		return target
	elseif op == "select" then
		local selected = {}
		for _, path in ipairs(action.targets or {}) do
			local target = resolvePath(path, refs)
			if target then
				table.insert(selected, target)
			end
		end
		Selection:Set(selected)
		return #selected
	elseif op == "add_tag" then
		local target = resolvePath(action.target or action.path, refs)
		assert(target, "add_tag target could not be resolved")
		CollectionService:AddTag(target, action.tag)
		return target
	elseif op == "remove_tag" then
		local target = resolvePath(action.target or action.path, refs)
		assert(target, "remove_tag target could not be resolved")
		CollectionService:RemoveTag(target, action.tag)
		return target
	elseif op == "pivot_to" then
		local target = resolvePath(action.target or action.path, refs)
		local pivot = decodeValue(action.cframe, refs)
		assert(target and typeof(pivot) == "CFrame", "pivot_to requires a target and CFrame")
		if target:IsA("Model") then
			target:PivotTo(pivot)
		elseif target:IsA("BasePart") then
			target.CFrame = pivot
		else
			error("pivot_to target must be Model or BasePart")
		end
		return target
	elseif op == "terrain_fill_block" then
		local cframe = decodeValue(action.cframe, refs)
		local size = decodeValue(action.size, refs)
		local material = decodeValue(action.material, refs)
		assert(typeof(cframe) == "CFrame" and typeof(size) == "Vector3", "terrain_fill_block requires CFrame and Vector3")
		assert(typeof(material) == "EnumItem", "terrain_fill_block requires Enum.Material")
		workspace.Terrain:FillBlock(cframe, size, material)
		return workspace.Terrain
	elseif op == "terrain_fill_ball" then
		local position = decodeValue(action.position, refs)
		local material = decodeValue(action.material, refs)
		assert(typeof(position) == "Vector3", "terrain_fill_ball requires Vector3 position")
		assert(typeof(material) == "EnumItem", "terrain_fill_ball requires Enum.Material")
		workspace.Terrain:FillBall(position, tonumber(action.radius) or 4, material)
		return workspace.Terrain
	elseif op == "terrain_clear" then
		workspace.Terrain:Clear()
		return workspace.Terrain
	end
	error("unsupported Studio mutation op " .. tostring(op))
end

local function executeBatch(command)
	local payload = command.payload or {}
	local actions = payload.actions or {}
	assert(#actions > 0, "studio_batch requires at least one action")
	local labelText = tostring(payload.label or "Roldex Studio changes")
	local liveDelay = math.clamp(tonumber(payload.live_delay_ms) or 55, 0, 500) / 1000
	local highlightCreated = payload.highlight_created ~= false
	local recording = ChangeHistoryService:TryBeginRecording("Roldex " .. command.id, labelText)
	assert(recording, "could not begin Studio undo recording")

	local refs = {}
	local results = {}
	local ok, errorMessage = pcall(function()
		for index, action in ipairs(actions) do
			setStatus(("Building %d/%d · %s"):format(index, #actions, tostring(action.op)), "neutral")
			local result = executeMutationAction(action, refs)
			table.insert(results, { index = index, op = action.op, result = encodeValue(result) })
			if highlightCreated and typeof(result) == "Instance" and (action.op == "create" or action.op == "clone") then
				Selection:Set({ result })
			end
			if liveDelay > 0 and index < #actions then
				task.wait(liveDelay)
			end
		end
	end)

	if ok then
		ChangeHistoryService:FinishRecording(recording, Enum.FinishRecordingOperation.Commit)
		setStatus(("Built %d Studio action%s"):format(#actions, #actions == 1 and "" or "s"), "ok")
		return { actions_completed = #actions, results = results }
	end
	pcall(function()
		ChangeHistoryService:FinishRecording(recording, Enum.FinishRecordingOperation.Cancel)
	end)
	error(errorMessage)
end

local function captureStudioView(payload)
	payload = payload or {}
	if not StudioCaptureService:CanCaptureScreenshot() then
		local allowed = StudioCaptureService:RequestScreenshotPermissionAsync()
		assert(allowed, "Studio screenshot permission was not granted")
	end
	local capture = StudioCaptureService:CaptureScreenshot({
		Format = Enum.StudioCaptureScreenshotFormat.PNG,
		UICaptureMode = payload.include_ui == true and Enum.UICaptureMode.All or Enum.UICaptureMode.None,
	})
	assert(capture, "Studio screenshot capture did not return a capture object")

	local errors = capture:GetErrors()
	if #errors > 0 then
		local formatted = {}
		for _, captureError in ipairs(errors) do
			table.insert(formatted, tostring(captureError))
		end
		error("Studio screenshot capture failed: " .. table.concat(formatted, "; "))
	end

	local width = math.clamp(tonumber(payload.width) or 960, 160, 1280)
	local height = math.clamp(tonumber(payload.height) or 540, 90, 720)
	if capture.Resolution.X ~= width or capture.Resolution.Y ~= height then
		capture = capture:ScaleAsync(Enum.ResamplerMode.Default, Vector2.new(width, height))
		local scaledErrors = capture:GetErrors()
		if #scaledErrors > 0 then
			local formatted = {}
			for _, captureError in ipairs(scaledErrors) do
				table.insert(formatted, tostring(captureError))
			end
			error("Studio screenshot scaling failed: " .. table.concat(formatted, "; "))
		end
	end

	local raw = capture:GetBuffer()
	local encodedBuffer = EncodingService:Base64Encode(raw)
	return {
		data_base64 = buffer.tostring(encodedBuffer),
		buffer_format = tostring(capture.BufferFormat),
		resolution = encodeValue(capture.Resolution),
		original_size = encodeValue(capture.OriginalSize),
		ui_capture_mode = tostring(capture.UICaptureMode),
	}
end

local function executeDevice(payload)
	payload = payload or {}
	local operation = payload.operation
	if operation == "status" then
		return {
			device_id = StudioDeviceSimulatorService:GetDeviceAsync(),
			resolution = encodeValue(StudioDeviceSimulatorService:GetResolutionAsync()),
			orientation = tostring(StudioDeviceSimulatorService:GetOrientationAsync()),
			pixel_density = StudioDeviceSimulatorService:GetPixelDensityAsync(),
			scaling_mode = tostring(StudioDeviceSimulatorService:GetScalingModeAsync()),
		}
	elseif operation == "list" then
		local ids = StudioDeviceSimulatorService:GetDeviceListAsync()
		local devices = {}
		for index, deviceId in ipairs(ids) do
			if index > MAX_DEVICE_RESULTS then
				break
			end
			local ok, info = pcall(function()
				return StudioDeviceSimulatorService:GetDeviceInfoAsync(deviceId)
			end)
			table.insert(devices, { id = deviceId, info = ok and info or nil })
		end
		return { devices = devices, truncated = #ids > MAX_DEVICE_RESULTS }
	elseif operation == "set_device" then
		assert(type(payload.device_id) == "string" and payload.device_id ~= "", "set_device requires device_id")
		StudioDeviceSimulatorService:SetDeviceAsync(payload.device_id)
		return executeDevice({ operation = "status" })
	elseif operation == "set_resolution" then
		local width = math.clamp(tonumber(payload.width) or 0, 240, 7680)
		local height = math.clamp(tonumber(payload.height) or 0, 240, 4320)
		StudioDeviceSimulatorService:SetResolutionAsync(width, height)
		return executeDevice({ operation = "status" })
	elseif operation == "set_orientation" then
		local mapping = {
			landscape = Enum.ScreenOrientation.LandscapeSensor,
			portrait = Enum.ScreenOrientation.Portrait,
			sensor = Enum.ScreenOrientation.Sensor,
		}
		local orientation = mapping[string.lower(tostring(payload.orientation or ""))]
		assert(orientation, "set_orientation requires landscape, portrait, or sensor")
		StudioDeviceSimulatorService:SetOrientationAsync(orientation)
		return executeDevice({ operation = "status" })
	elseif operation == "stop" then
		StudioDeviceSimulatorService:StopSimulationAsync()
		return { stopped = true }
	end
	error("unsupported Studio device operation " .. tostring(operation))
end

local function pointerActionFromJson(value)
	local output = {}
	if type(value) ~= "table" then
		return output
	end
	if tonumber(value.Wheel or value.wheel) then
		output.Wheel = tonumber(value.Wheel or value.wheel)
	end
	if tonumber(value.Pinch or value.pinch) then
		output.Pinch = tonumber(value.Pinch or value.pinch)
	end
	local pan = value.Pan or value.pan
	if type(pan) == "table" then
		output.Pan = Vector2.new(tonumber(pan.x or pan.X) or 0, tonumber(pan.y or pan.Y) or 0)
	end
	return output
end

local function runInputStep(virtualInput, step)
	local kind = step.type
	if kind == "key" then
		local keyCode = Enum.KeyCode[step.key_code]
		assert(keyCode, "unknown KeyCode " .. tostring(step.key_code))
		virtualInput:SendKey(true, keyCode, false)
		local hold = math.clamp(tonumber(step.hold_seconds) or 0.05, 0, 10)
		if hold > 0 then
			task.wait(hold)
		end
		virtualInput:SendKey(false, keyCode, false)
	elseif kind == "mouse_button" then
		local buttonName = tostring(step.button or "MouseButton1")
		local button = Enum.UserInputType[buttonName]
		assert(button, "unknown mouse button " .. buttonName)
		local position = Vector2.new(tonumber(step.x) or 0, tonumber(step.y) or 0)
		virtualInput:SendMouseButton(position, button, true, 0)
		local hold = math.clamp(tonumber(step.hold_seconds) or 0.05, 0, 10)
		if hold > 0 then
			task.wait(hold)
		end
		virtualInput:SendMouseButton(position, button, false, 0)
	elseif kind == "mouse_move" then
		virtualInput:SendMouseDelta(Vector2.new(tonumber(step.x) or 0, tonumber(step.y) or 0))
	elseif kind == "mouse_position" then
		virtualInput:SendMousePosition(Vector2.new(tonumber(step.x) or 0, tonumber(step.y) or 0))
	elseif kind == "text" then
		virtualInput:SendTextInput(tostring(step.text or ""))
	elseif kind == "pointer" then
		virtualInput:SendPointerAction(
			Vector2.new(tonumber(step.x) or 0, tonumber(step.y) or 0),
			pointerActionFromJson(step.pointer_action)
		)
	else
		error("unsupported virtual input step " .. tostring(kind))
	end
end

local function runStudioTest(payload)
	payload = payload or {}
	local mode = payload.mode or "run"
	local timeoutSeconds = math.clamp(tonumber(payload.timeout_seconds) or 12, 2, 90)
	local players = math.clamp(tonumber(payload.players) or 2, 1, 8)
	local logs = {}
	local warningCount = 0
	local errorCount = 0
	local inputErrors = {}
	local inputEvents = 0
	local testCapture = nil

	local logConnection = LogService.MessageOut:Connect(function(message, messageType)
		if #logs >= MAX_LOGS then
			return
		end
		if messageType == Enum.MessageType.MessageWarning then
			warningCount += 1
		elseif messageType == Enum.MessageType.MessageError then
			errorCount += 1
		end
		table.insert(logs, { message = shortValue(message, 1500), message_type = tostring(messageType) })
	end)

	local inputSteps = payload.input_steps or {}
	table.sort(inputSteps, function(a, b)
		return (tonumber(a.at_seconds) or 0) < (tonumber(b.at_seconds) or 0)
	end)
	local inputTask = nil
	if #inputSteps > 0 then
		inputTask = task.spawn(function()
			local startTime = os.clock()
			local virtualInput = nil
			for _, step in ipairs(inputSteps) do
				local atSeconds = math.clamp(tonumber(step.at_seconds) or 0.5, 0, timeoutSeconds - 0.1)
				local waitTime = atSeconds - (os.clock() - startTime)
				if waitTime > 0 then
					task.wait(waitTime)
				end
				if not virtualInput then
					local ok, created = pcall(function()
						return UserInputService:CreateVirtualInput()
					end)
					if ok then
						virtualInput = created
					end
					if not virtualInput then
						table.insert(inputErrors, "VirtualInput is unavailable in this Studio context")
						break
					end
				end
				local ok, errorMessage = pcall(function()
					runInputStep(virtualInput, step)
				end)
				if ok then
					inputEvents += 1
				else
					table.insert(inputErrors, tostring(errorMessage))
				end
			end
		end)
	end

	local captureTask = nil
	local captureAt = tonumber(payload.capture_at_seconds)
	if captureAt then
		captureAt = math.clamp(captureAt, 0.2, timeoutSeconds - 0.1)
		captureTask = task.spawn(function()
			task.wait(captureAt)
			local ok, captureOrError = pcall(function()
				return captureStudioView({ width = 960, height = 540, include_ui = true })
			end)
			if ok then
				testCapture = captureOrError
			else
				testCapture = { error = tostring(captureOrError) }
			end
		end)
	end

	setStatus(("Testing game · %s mode"):format(mode), "neutral")
	local stopper = task.delay(timeoutSeconds, function()
		pcall(function()
			if StudioTestService:CanLeaveTest() then
				StudioTestService:LeaveTest()
			end
		end)
	end)

	local ok, result = pcall(function()
		local args = payload.args or { roldex = true, timeout_seconds = timeoutSeconds }
		if mode == "run" then
			return StudioTestService:ExecuteRunModeAsync(args)
		elseif mode == "play" then
			return StudioTestService:ExecutePlayModeAsync(args)
		elseif mode == "multiplayer" then
			return StudioTestService:ExecuteMultiplayerTestAsync(players, args)
		end
		error("unsupported Studio test mode " .. tostring(mode))
	end)

	pcall(function()
		task.cancel(stopper)
	end)
	if inputTask then
		pcall(function()
			task.cancel(inputTask)
		end)
	end
	if captureTask and not testCapture then
		pcall(function()
			task.cancel(captureTask)
		end)
	end
	logConnection:Disconnect()
	if not ok then
		error(result)
	end

	setStatus(
		("Test finished · %d error%s, %d warning%s"):format(
			errorCount,
			errorCount == 1 and "" or "s",
			warningCount,
			warningCount == 1 and "" or "s"
		),
		errorCount > 0 and "error" or "ok"
	)

	return {
		mode = mode,
		players = mode == "multiplayer" and players or nil,
		result = encodeValue(result),
		error_count = errorCount,
		warning_count = warningCount,
		logs = logs,
		log_truncated = #logs >= MAX_LOGS,
		input_events_sent = inputEvents,
		input_errors = inputErrors,
		capture = testCapture,
	}
end

local function executeCommand(command)
	local ok, output = pcall(function()
		if command.action == "query" then
			return executeQuery(command.payload or {})
		elseif command.action == "batch" then
			return executeBatch(command)
		elseif command.action == "capture" then
			return captureStudioView(command.payload or {})
		elseif command.action == "device" then
			return executeDevice(command.payload or {})
		elseif command.action == "test" then
			return runStudioTest(command.payload or {})
		elseif command.action == "undo" then
			ChangeHistoryService:Undo()
			return { undone = true }
		elseif command.action == "redo" then
			ChangeHistoryService:Redo()
			return { redone = true }
		end
		error("unsupported Studio command action " .. tostring(command.action))
	end)
	if ok then
		return { id = command.id, ok = true, output = output }
	end
	setStatus("Studio action failed · Roldex will try to repair it", "error")
	return { id = command.id, ok = false, error = tostring(output) }
end

local function postCommandResult(result)
	return request({
		Url = bridgeUrl() .. "/v1/actions/result",
		Method = "POST",
		Headers = { ["Content-Type"] = "application/json" },
		Body = HttpService:JSONEncode(result),
	})
end

local function pollCommands()
	if pollBusy then
		return
	end
	pollBusy = true
	local ok, response = pcall(function()
		return request({ Url = bridgeUrl() .. "/v1/actions", Method = "GET" })
	end)
	if ok and response.Success then
		pluginConnected = true
		local decodedOk, decoded = pcall(function()
			return HttpService:JSONDecode(response.Body)
		end)
		if decodedOk and decoded.ok and type(decoded.commands) == "table" then
			for _, command in ipairs(decoded.commands) do
				local result = executeCommand(command)
				local postOk, postResponse = pcall(function()
					return postCommandResult(result)
				end)
				if not postOk or not postResponse.Success then
					pluginConnected = false
					break
				end
			end
		end
	else
		pluginConnected = false
	end
	pollBusy = false
end

local function checkHealth()
	setStatus("Checking bridge...", "neutral")
	local ok, response = pcall(function()
		return request({ Url = bridgeUrl() .. "/health", Method = "GET" })
	end)
	if ok and response.Success then
		pluginConnected = true
		setStatus("Connected · live Studio control ready", "ok")
	else
		pluginConnected = false
		setStatus("Disconnected — start `roldex` in your project", "error")
	end
end

local function sendPrompt()
	if not sendButton.Active then
		return
	end
	local message = promptBox.Text:gsub("%s+$", ""):gsub("^%s+", "")
	if message == "" then
		setStatus("Write a prompt first", "error")
		return
	end
	setButtonEnabled(sendButton, false)
	sendButton.Text = "Roldex is working..."
	setStatus("Researching / building / testing / verifying...", "neutral")
	local payload = {
		message = message,
		selection = collectSelection(),
		active_script = collectActiveScript(),
	}
	local ok, response = pcall(function()
		return request({
			Url = bridgeUrl() .. "/v1/chat",
			Method = "POST",
			Headers = { ["Content-Type"] = "application/json" },
			Body = HttpService:JSONEncode(payload),
		})
	end)
	if not ok or not response.Success then
		outputLabel.Text = "Roldex request failed: " .. responseDetail(response)
		setStatus("Request failed", "error")
	else
		local decodedOk, decoded = pcall(function()
			return HttpService:JSONDecode(response.Body)
		end)
		if decodedOk and decoded.ok and type(decoded.answer) == "string" then
			outputLabel.Text = decoded.answer
			setStatus("Finished · verified result returned", "ok")
		else
			outputLabel.Text = "Roldex returned an invalid bridge response."
			setStatus("Invalid response", "error")
		end
	end
	setButtonEnabled(sendButton, true)
	sendButton.BackgroundColor3 = colors.accentDark
	sendButton.Text = "Build / Fix / Test"
end

bridgeUrlBox.FocusLost:Connect(function()
	local value = bridgeUrl()
	plugin:SetSetting("RoldexBridgeUrl", value)
	task.spawn(checkHealth)
end)
Selection.SelectionChanged:Connect(updateSelectionLabel)
StudioService:GetPropertyChangedSignal("ActiveScript"):Connect(updateSelectionLabel)
sendButton.Activated:Connect(function()
	task.spawn(sendPrompt)
end)
healthButton.Activated:Connect(function()
	task.spawn(checkHealth)
end)
toolbarButton.Click:Connect(function()
	widget.Enabled = not widget.Enabled
end)
widget:GetPropertyChangedSignal("Enabled"):Connect(function()
	toolbarButton:SetActive(widget.Enabled)
	if widget.Enabled then
		updateSelectionLabel()
		task.spawn(checkHealth)
	end
end)

task.spawn(function()
	while true do
		task.wait(pluginConnected and 0.12 or 0.75)
		pollCommands()
	end
end)

updateSelectionLabel()
task.spawn(checkHealth)
