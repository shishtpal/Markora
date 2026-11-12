param([int]$ProcessId, [ValidateSet('Inspect', 'F11', 'F11Repeat', 'F11Parent', 'Escape', 'Maximize', 'Restore')][string]$Action = 'Inspect')
$ErrorActionPreference = 'Stop'
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class ReadingWindow {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int left, top, right, bottom; }
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int x, y; }
  [StructLayout(LayoutKind.Sequential)] public struct PLACEMENT {
    public int length, flags, showCmd; public POINT min, max; public RECT normal;
  }
  [StructLayout(LayoutKind.Sequential)] public struct MONITOR {
    public int size; public RECT monitor, work; public int flags;
  }
  [StructLayout(LayoutKind.Sequential)] public struct GUI {
    public int size, flags; public IntPtr active, focus, capture, menu, move, caret; public RECT rect;
  }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out RECT rect);
  [DllImport("user32.dll")] public static extern bool GetWindowPlacement(IntPtr hwnd, ref PLACEMENT placement);
  [DllImport("user32.dll")] public static extern IntPtr MonitorFromWindow(IntPtr hwnd, int flags);
  [DllImport("user32.dll")] public static extern bool GetMonitorInfo(IntPtr monitor, ref MONITOR info);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern bool GetGUIThreadInfo(uint thread, ref GUI info);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint process);
  [DllImport("user32.dll")] public static extern bool IsChild(IntPtr parent, IntPtr child);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd, uint msg, IntPtr key, IntPtr data);
  [DllImport("user32.dll")] public static extern uint MapVirtualKey(uint key, uint map);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hwnd, int cmd);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
}
'@
$null = [ReadingWindow]::SetProcessDPIAware()
$window = (Get-Process -Id $ProcessId).MainWindowHandle
if ($window -eq [IntPtr]::Zero) { throw 'Smoke-test process has no native window' }
if ($Action -eq 'Maximize' -or $Action -eq 'Restore') {
  $command = if ($Action -eq 'Maximize') { 3 } else { 9 }
  $null = [ReadingWindow]::ShowWindow($window, $command)
} elseif ($Action -ne 'Inspect') {
  $null = [ReadingWindow]::SetForegroundWindow($window)
  $gui = New-Object ReadingWindow+GUI
  $gui.size = [Runtime.InteropServices.Marshal]::SizeOf($gui)
  [uint32]$ownerProcess = 0
  $thread = [ReadingWindow]::GetWindowThreadProcessId($window, [ref]$ownerProcess)
  if (![ReadingWindow]::GetGUIThreadInfo($thread, [ref]$gui)) { throw 'Cannot inspect keyboard focus' }
  if ($gui.focus -ne $window -and ![ReadingWindow]::IsChild($window, $gui.focus)) {
    throw 'Keyboard focus is outside the isolated smoke-test window'
  }
  if ($Action -eq 'F11Parent') { $gui.focus = $window }
  $key = if ($Action -like 'F11*') { 0x7A } else { 0x1B }
  $data = ([ReadingWindow]::MapVirtualKey($key, 0) -shl 16) -bor 1
  if ($Action -eq 'F11Repeat') { $data = $data -bor 0x40000000 }
  $null = [ReadingWindow]::PostMessage($gui.focus, 0x100, [IntPtr]$key, [IntPtr]$data)
  $null = [ReadingWindow]::PostMessage($gui.focus, 0x101, [IntPtr]$key, [IntPtr]($data -bor [int]::MinValue -bor 0x40000000))
}
$rect = New-Object ReadingWindow+RECT
$placement = New-Object ReadingWindow+PLACEMENT
$placement.length = [Runtime.InteropServices.Marshal]::SizeOf($placement)
$monitor = New-Object ReadingWindow+MONITOR
$monitor.size = [Runtime.InteropServices.Marshal]::SizeOf($monitor)
if (![ReadingWindow]::GetWindowRect($window, [ref]$rect)) { throw 'Cannot inspect window bounds' }
if (![ReadingWindow]::GetWindowPlacement($window, [ref]$placement)) { throw 'Cannot inspect window placement' }
if (![ReadingWindow]::GetMonitorInfo([ReadingWindow]::MonitorFromWindow($window, 2), [ref]$monitor)) { throw 'Cannot inspect monitor' }
@{ rect = $rect; placement = $placement; monitor = $monitor.monitor } | ConvertTo-Json -Compress -Depth 5
