assert(plugin, "Roldex Studio must run as a Roblox Studio plugin")

local HttpService = game:GetService("HttpService")
local Selection = game:GetService("Selection")
local ScriptEditorService = game:GetService("ScriptEditorService")
local StudioService = game:GetService("StudioService")

local DEFAULT_BRIDGE_URL = "http://127.0.0.1:38247"
local MAX_SELECTION_ITEMS = 20
local MAX_SOURCE_CHARS = 60000
local MAX_ATTRIBUTES = 20
local MAX_DETAIL_CHARS = 2600

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
promptBox.PlaceholderText = "Ask about the selected object, active script, bug, UI, map, animation, or anything else in this Roblox project..."
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
	"Send with Studio Context",
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
outputLabel.Text = "Roldex answers will appear here."
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

local function bridgeUrl()
	local value = bridgeUrlBox.Text:gsub("%s+$", ""):gsub("^%s+", "")
	if value == "" then
		value = DEFAULT_BRIDGE_URL
	end
	return value:gsub("/$", "")
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

local function checkHealth()
	setStatus("Checking bridge...", "neutral")
	local ok, response = pcall(function()
		return request({
			Url = bridgeUrl() .. "/health",
			Method = "GET",
		})
	end)

	if ok and response.Success then
		setStatus("Connected to local Roldex CLI", "ok")
	else
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
	setStatus("Sending live Studio context...", "neutral")

	local payload = {
		message = message,
		selection = collectSelection(),
		active_script = collectActiveScript(),
	}

	local ok, response = pcall(function()
		return request({
			Url = bridgeUrl() .. "/v1/chat",
			Method = "POST",
			Headers = {
				["Content-Type"] = "application/json",
			},
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
			setStatus("Connected — answer received", "ok")
		else
			lastAnswer = ""
			outputLabel.Text = "Roldex returned an invalid bridge response."
			setStatus("Invalid response", "error")
		end
	end

	refreshApplyState()
	setButtonEnabled(sendButton, true)
	sendButton.BackgroundColor3 = colors.accentDark
	sendButton.Text = "Send with Studio Context"
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
	local ok, errorMessage = pcall(function()
		ScriptEditorService:UpdateSourceAsync(active, function()
			return code
		end)
	end)

	if ok then
		setStatus("Applied Luau block to " .. active.Name, "ok")
	else
		setStatus("Apply failed: " .. tostring(errorMessage), "error")
	end
	refreshApplyState()
end

bridgeUrlBox.FocusLost:Connect(function()
	plugin:SetSetting("RoldexBridgeUrl", bridgeUrl())
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

updateSelectionLabel()
refreshApplyState()
