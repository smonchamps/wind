# REAL toggle of the Windows dark theme: registry + broadcast
# WM_SETTINGCHANGE "ImmersiveColorSet" -- what Settings does.
# Set-ItemProperty alone notifies no one (measured at the field-finding
# probes A42, 2026-08-16): without the broadcast, neither the Tauri API
# nor any application sees the toggle. Used by the "OS follow" test of
# redesign-screen02.spec.js, Windows only.
param([Parameter(Mandatory)][int]$v)

Set-ItemProperty HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Themes\Personalize -Name AppsUseLightTheme -Value $v -Type DWord
Add-Type -Namespace Wind -Name Diffuse -MemberDefinition '[DllImport("user32.dll", CharSet = CharSet.Auto)] public static extern IntPtr SendMessageTimeout(IntPtr hWnd, uint Msg, UIntPtr wParam, string lParam, uint fuFlags, uint uTimeout, out UIntPtr lpdwResult);'
[UIntPtr]$r = [UIntPtr]::Zero
# 0xffff = HWND_BROADCAST, 0x001A = WM_SETTINGCHANGE, 2 = SMTO_ABORTIFHUNG
[Wind.Diffuse]::SendMessageTimeout([IntPtr]0xffff, 0x001A, [UIntPtr]::Zero, "ImmersiveColorSet", 2, 5000, [ref]$r) | Out-Null
