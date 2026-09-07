assert(plugin, "Roldex Studio must run as a Roblox Studio plugin")

local HttpService = game:GetService("HttpService")
local Selection = game:GetService("Selection")
local ScriptEditorService = game:GetService("ScriptEditorService")
local StudioService = game:GetService("StudioService")
local ChangeHistoryService = game:GetService("ChangeHistoryService")
local CollectionService = game:GetService("CollectionService")
local StudioTestService = game:GetService("StudioTestService")
local LogService = game:GetService("LogService")

local DEFAULT_BRIDGE_URL = "http://127.0.0.1:38247"
local MAX_SELECTION_ITEMS = 20
local MAX_SOURCE_CHARS = 60000
local MAX_ATTRIBUTES = 20
local MAX_DETAIL_CHARS = 2600
local MAX_QUERY_RESULTS = 100
local MAX_LOGS = 200

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
	440,
	650,
	340,
	460
)

local widget = plugin:CreateDockWidgetPluginGuiAsync("RoldexStudioDock", widgetInfo)
widget.Title = "Roldex Studio"

local root = Instance.new("Frame")
root.Name = "Root"
root.Size = UDim2.fromScale(1, 1)
root.BackgroundColor3 = colors.background
root.BorderSizePixel = 0
root.Parent = widget

local padding = Instance.new("UIPadding")
padding.PaddingTop = UDim.new(0, 12)
padding.PaddingBottom = UDim.new(0, 12)
padding.PaddingLeft = UDim.new(0, 12)
padding.PaddingRight = UDim.new(0, 12)
padding.Parent = root

local function label(name, text, position, size, textSize, color)
	local instance = Instance.new("TextLabel")
	instance.Name = name
	instance.BackgroundTransparency = 1
	instance.Position = position
	instance.Size = size
	instance.Font = Enum.Font.Gotham
	instance.Text = text
	instance.TextSize = textSize
	instance.TextColor3 = color or colors.text
	instance.TextXAlignment = Enum.TextXAlignment.Left
	instance.TextYAlignment = Enum.TextYAlignment.Center
	instance.Parent = root
	return instance
end

local function button(name, text, position, size)
	local instance = Instance.new("TextButton")
	instance.Name = name
	instance.Position = position
	instance.Size = size
	instance.BackgroundColor3 = colors.panelAlt
	instance.BorderSizePixel = 0
	instance.Active = true
	instance.AutoButtonColor = true
	instance.Font = Enum.Font.GothamSemibold
	instance.Text = text
	instance.TextSize = 13
	instance.TextColor3 = colors.text
	instance.Parent = root

	local corner = Instance.new("UICorner")
	corner.CornerRadius = UDim.new(0, 6)
	corner.Parent = instance
	return instance
end

local function setButtonEnabled(instance, enabled)
	instance.Active = enabled
	instance.AutoButtonColor = enabled
	instance.TextColor3 = enabled and colors.text or colors.muted
end

local title = label(
	"Title",
	"ROLDEX",
	UDim2.new(0, 0, 0, 0),
	UDim2.new(1, 0, 0, 28),
	20,
	colors.text
)
title.Font = Enum.Font.GothamBold

local statusLabel = label(
	"Status",
	"● Checking bridge...",
	UDim2.new(0, 0, 0, 30),
	UDim2.new(1, 0, 0, 22),
	12,
	colors.muted
)

local selectionLabel = label(
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
promptBox.Size = UDim2.new(1, 0, 0, 120)
promptBox.BackgroundColor3 = colors.panel
promptBox.BorderSizePixel = 0
promptBox.ClearTextOnFocus = false
promptBox.Font = Enum.Font.Code
promptBox.MultiLine = true
promptBox.PlaceholderText = "Describe the finished Roblox result you want. Roldex can build, script, test, inspect, repair, and keep going until it finishes..."
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

local sendButton = button(
	"Send",
	"Build / Fix with Roldex",
	UDim2.new(0, 0, 0, 270),
	UDim2.new(0.68, -4, 0, 34)
)
sendButton.BackgroundColor3 = colors.accentDark

local healthButton = button(
	"Reconnect",
	"Reconnect",
	UDim2.new(0.68, 4, 0, 270),
	UDim2.new(0.32, -4, 0, 34)
)

local outputFrame = Instance.new("ScrollingFrame")
outputFrame.Name = "Output"
outputFrame.Position = UDim2.new(0, 0, 0, 314)
outputFrame.Size = UDim2.new(1, 0, 1, -362)
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
outputLabel.Text = "Roldex is ready. Keep Studio open and you can watch live changes appear here and in the 3D view/Explorer."
outputLabel.TextColor3 = colors.text
outputLabel.TextSize = 13
outputLabel.TextWrapped = true
outputLabel.TextXAlignment = Enum.TextXAlignment.Left
outputLabel.TextYAlignment = Enum.TextYAlignment.Top
outputLabel.Parent = outputFrame

local applyButton = button(
	"Apply",
	"Apply First Luau Block to Active Script",
	UDim2.new(0, 0, 1, -38),
	UDim2.new(1, 0, 0, 34)
)
setButtonEnabled(applyButton, false)

local lastAnswer = ""
local pollBusy = false
local pluginConnected = false

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

local function updateSelectionLabel()
	local selected = Selection:Get()
	local active = StudioService.ActiveScript
	local parts = { ("Selection: %d"):format(#selected) }
	if isScriptContainer(active) then
		table.insert(parts, "Active: " .. active:GetFullName())
	end
	selectionLabel.Text = table.concat(parts, "  |  ")
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
	end
	if path == "$activeScript" then
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
		if descendant:GetFullName() == original or descendant:GetFullName() == path then
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
		local components = value.components or {}
		return CFrame.new(table.unpack(components))
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
			table.insert(keypoints, ColorSequenceKeypoint.new(
				keypoint.time,
				Color3.new(keypoint.r or 0, keypoint.g or 0, keypoint.b or 0)
			))
		end
		return ColorSequence.new(keypoints)
	end
	error("unsupported typed Studio value " .. tostring(typeName))
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

local function updateScriptSource(script, source)
	assert(isScriptContainer(script), "target is not a Script, LocalScript, or ModuleScript")
	assert(type(source) == "string", "script source must be a string")
	ScriptEditorService:UpdateSourceAsync(script, function()
		return source
	end)
end

local function serializeInstance(instance, requestedProperties)
	local properties = {}
	for _, property in ipairs(requestedProperties or {}) do
		local ok, value = pcall(function()
			return instance[property]
		end)
		if ok then
			properties[property] = encodeValue(value)
		else
			properties[property] = "<unreadable>"
		end
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
			if #items >= (payload.max_results or MAX_QUERY_RESULTS) then
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
		local candidates = rootInstance == game and game:GetDescendants() or rootInstance:GetDescendants()
		for _, instance in ipairs(candidates) do
			local nameMatches = nameContains == "" or string.find(string.lower(instance.Name), nameContains, 1, true) ~= nil
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
		assert(typeof(pivot) == "CFrame", "pivot_to requires a CFrame")
		assert(target, "pivot_to target could not be resolved")
		if target:IsA("Model") then
			target:PivotTo(pivot)
		elseif target:IsA("BasePart") then
			target.CFrame = pivot
		else
			error("pivot_to target must be Model or BasePart")
		end
		return target
	elseif op == "terrain_fill_block" then
		local terrain = workspace.Terrain
		local cframe = decodeValue(action.cframe, refs)
		local size = decodeValue(action.size, refs)
		local material = decodeValue(action.material, refs)
		assert(typeof(cframe) == "CFrame" and typeof(size) == "Vector3", "terrain_fill_block requires CFrame and Vector3")
		assert(typeof(material) == "EnumItem", "terrain_fill_block requires Enum.Material")
		terrain:FillBlock(cframe, size, material)
		return terrain
	elseif op == "terrain_fill_ball" then
		local terrain = workspace.Terrain
		local position = decodeValue(action.position, refs)
		local material = decodeValue(action.material, refs)
		assert(typeof(position) == "Vector3", "terrain_fill_ball requires Vector3 position")
		terrain:FillBall(position, tonumber(action.radius) or 4, material)
		return terrain
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
			local encoded = encodeValue(result)
			table.insert(results, { index = index, op = action.op, result = encoded })
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

local function runStudioTest(payload)
	local mode = payload.mode or "run"
	local timeoutSeconds = math.clamp(tonumber(payload.timeout_seconds) or 10, 2, 60)
	local players = math.clamp(tonumber(payload.players) or 2, 1, 8)
	local logs = {}
	local warningCount = 0
	local errorCount = 0
	local connection = LogService.MessageOut:Connect(function(message, messageType)
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
	connection:Disconnect()
	if not ok then
		error(result)
	end

	setStatus(("Test finished · %d error%s, %d warning%s"):format(
		errorCount,
		errorCount == 1 and "" or "s",
		warningCount,
		warningCount == 1 and "" or "s"
	), errorCount > 0 and "error" or "ok")

	return {
		mode = mode,
		players = mode == "multiplayer" and players or nil,
		result = encodeValue(result),
		error_count = errorCount,
		warning_count = warningCount,
		logs = logs,
		log_truncated = #logs >= MAX_LOGS,
	}
end

local function executeCommand(command)
	local ok, output = pcall(function()
		if command.action == "query" then
			return executeQuery(command.payload or {})
		elseif command.action == "batch" then
			return executeBatch(command)
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
				local postOk = pcall(function()
					postCommandResult(result)
				end)
				if not postOk then
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

local function extractLuauCode(text)
	return text:match("```luau%s*\n(.-)\n```")
		or text:match("```lua%s*\n(.-)\n```")
		or text:match("```%s*\n(.-)\n```")
end

local function refreshApplyState()
	local canApply = isScriptContainer(StudioService.ActiveScript) and extractLuauCode(lastAnswer) ~= nil
	setButtonEnabled(applyButton, canApply)
	applyButton.BackgroundColor3 = canApply and colors.accentDark or colors.panelAlt
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
	setStatus("Inspecting, building and verifying...", "neutral")

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
		lastAnswer = ""
		outputLabel.Text = "Roldex request failed: " .. responseDetail(response)
		setStatus("Request failed", "error")
	else
		local decodedOk, decoded = pcall(function()
			return HttpService:JSONDecode(response.Body)
		end)
		if decodedOk and decoded.ok and type(decoded.answer) == "string" then
			lastAnswer = decoded.answer
			outputLabel.Text = decoded.answer
			setStatus("Finished · verified result returned", "ok")
		else
			lastAnswer = ""
			outputLabel.Text = "Roldex returned an invalid bridge response."
			setStatus("Invalid response", "error")
		end
	end

	refreshApplyState()
	setButtonEnabled(sendButton, true)
	sendButton.BackgroundColor3 = colors.accentDark
	sendButton.Text = "Build / Fix with Roldex"
end

local function applyFirstCodeBlock()
	if not applyButton.Active then
		return
	end

	local active = StudioService.ActiveScript
	local code = extractLuauCode(lastAnswer)
	if not isScriptContainer(active) or not code then
		refreshApplyState()
		return
	end

	setButtonEnabled(applyButton, false)
	local recording = ChangeHistoryService:TryBeginRecording("Roldex manual code apply", "Roldex: Apply code")
	local ok, errorMessage = pcall(function()
		ScriptEditorService:UpdateSourceAsync(active, function()
			return code
		end)
	end)

	if recording then
		ChangeHistoryService:FinishRecording(
			recording,
			ok and Enum.FinishRecordingOperation.Commit or Enum.FinishRecordingOperation.Cancel
		)
	end
	if ok then
		setStatus("Applied Luau block to " .. active.Name, "ok")
	else
		setStatus("Apply failed: " .. tostring(errorMessage), "error")
	end
	refreshApplyState()
end

bridgeUrlBox.FocusLost:Connect(function()
	local value = bridgeUrl()
	plugin:SetSetting("RoldexBridgeUrl", value)
	task.spawn(checkHealth)
end)

Selection.SelectionChanged:Connect(function()
	updateSelectionLabel()
	refreshApplyState()
end)

StudioService:GetPropertyChangedSignal("ActiveScript"):Connect(function()
	updateSelectionLabel()
	refreshApplyState()
end)

sendButton.Activated:Connect(function()
	task.spawn(sendPrompt)
end)

healthButton.Activated:Connect(function()
	task.spawn(checkHealth)
end)

applyButton.Activated:Connect(applyFirstCodeBlock)

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
refreshApplyState()
task.spawn(checkHealth)
