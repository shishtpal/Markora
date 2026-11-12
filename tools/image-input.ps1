param([ValidateSet('Clipboard', 'Drop', 'Save', 'Cancel')][string]$Action, [string]$ImagePath, [int]$ProcessId, [int]$X, [int]$Y, [string]$RestoreSignal, [string]$SavePath)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
function Backup-Clipboard {
  $original = [Windows.Forms.Clipboard]::GetDataObject()
  $backup = New-Object Windows.Forms.DataObject
  if ($null -ne $original) {
    foreach ($format in $original.GetFormats($false)) {
      try { $backup.SetData($format, $false, $original.GetData($format, $false)) } catch {}
    }
  }
  return ,$backup
}
if ($Action -eq 'Clipboard') {
  $backup = Backup-Clipboard
  $image = [Drawing.Image]::FromFile($ImagePath)
  try {
    [Windows.Forms.Clipboard]::SetImage($image)
    Write-Output 'READY'
    if ($RestoreSignal) {
      $deadline = [DateTime]::UtcNow.AddMinutes(2)
      while (!(Test-Path -LiteralPath $RestoreSignal) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 100 }
    }
  } finally {
    $image.Dispose()
    [Windows.Forms.Clipboard]::SetDataObject($backup, $true)
  }
  exit
}
if ($Action -eq 'Save' -or $Action -eq 'Cancel') {
  Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class ImageSaveDialog {
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr window, System.Text.StringBuilder name, int size);
}
'@
  $found = $false
  for ($attempt = 0; $attempt -lt 100; $attempt++) {
    $window = [ImageSaveDialog]::GetForegroundWindow()
    [uint32]$owner = 0
    $null = [ImageSaveDialog]::GetWindowThreadProcessId($window, [ref]$owner)
    $class = New-Object Text.StringBuilder 256
    $null = [ImageSaveDialog]::GetClassName($window, $class, 256)
    if ($owner -eq $ProcessId -and $class.ToString() -eq '#32770') { $found = $true; break }
    Start-Sleep -Milliseconds 100
  }
  if (!$found) { throw 'No save dialog owned by the isolated test process is focused' }
  if ($Action -eq 'Cancel') { [Windows.Forms.SendKeys]::SendWait('{ESC}'); exit }
  $backup = Backup-Clipboard
  try {
    [Windows.Forms.Clipboard]::SetText($SavePath)
    [Windows.Forms.SendKeys]::SendWait('%n')
    [Windows.Forms.SendKeys]::SendWait('^a^v')
    [Windows.Forms.SendKeys]::SendWait('{ENTER}')
    Start-Sleep -Milliseconds 200
  } finally { [Windows.Forms.Clipboard]::SetDataObject($backup, $true) }
  exit
}
Add-Type -ReferencedAssemblies System.Windows.Forms, System.Drawing @'
using System;
using System.Drawing;
using System.Threading;
using System.Windows.Forms;
using System.Runtime.InteropServices;
public static class ImageDropInput {
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int x, y; }
  [DllImport("user32.dll")] static extern bool ClientToScreen(IntPtr window, ref POINT point);
  [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr window);
  [DllImport("user32.dll")] static extern bool ShowWindow(IntPtr window, int command);
  [DllImport("user32.dll")] static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] static extern void mouse_event(uint flags, uint x, uint y, uint data, UIntPtr extra);
  public static void Drop(IntPtr target, string file, int x, int y) {
    ShowWindow(target, 5);
    POINT point = new POINT { x = x, y = y };
    if (!ClientToScreen(target, ref point)) throw new Exception("Could not locate isolated test window");
    using (Form source = new Form()) {
      source.Text = "Markora image drop test";
      source.StartPosition = FormStartPosition.Manual;
      source.Location = new Point(10, 10); source.Size = new Size(210, 80);
      source.TopMost = true;
      Exception failure = new Exception("The native drag source did not receive its mouse-down event");
      source.MouseDown += (sender, args) => {
        if (args.Button != MouseButtons.Left) return;
        DataObject data = new DataObject(DataFormats.FileDrop, new string[] { file });
        try {
          failure = source.DoDragDrop(data, DragDropEffects.Copy) == DragDropEffects.None
            ? new Exception("The native drag was not accepted by the test window") : null;
        } finally { mouse_event(4, 0, 0, 0, UIntPtr.Zero); source.Close(); }
      };
      System.Windows.Forms.Timer timeout = new System.Windows.Forms.Timer();
      timeout.Interval = 6000;
      timeout.Tick += (sender, args) => { SendKeys.SendWait("{ESC}"); source.Close(); };
      source.Shown += (sender, args) => {
        ShowWindow(source.Handle, 5);
        source.BringToFront();
        source.Activate();
        timeout.Start();
        POINT origin = new POINT { x = 70, y = 15 };
        ClientToScreen(source.Handle, ref origin);
        SetForegroundWindow(source.Handle);
        Thread move = new Thread(() => {
          Thread.Sleep(200); SetCursorPos(origin.x, origin.y);
          Thread.Sleep(200); mouse_event(2, 0, 0, 0, UIntPtr.Zero);
          Thread.Sleep(500);
          for (int step=1; step<=10; step++) {
            SetCursorPos(origin.x+(point.x-origin.x)*step/10, origin.y+(point.y-origin.y)*step/10);
            Thread.Sleep(60);
          }
          Thread.Sleep(400); mouse_event(4, 0, 0, 0, UIntPtr.Zero);
        });
        move.IsBackground = true; move.Start();
      };
      Application.Run(source);
      timeout.Dispose();
      if (failure != null) throw failure;
    }
  }
}
'@
$window = (Get-Process -Id $ProcessId).MainWindowHandle
if (!$window) { throw 'Isolated Markora test window is unavailable' }
[ImageDropInput]::Drop($window, $ImagePath, $X, $Y)
