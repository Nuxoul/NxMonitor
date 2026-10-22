param([int]$CapturePid = 0, [string]$PresentMon = '', [string]$Language = 'en', [string]$Style = 'modern', [string]$OutputDirectory = '')
$ErrorActionPreference = 'Stop'
$project = Split-Path $PSScriptRoot -Parent
if (!$OutputDirectory) {
 $root = if ($env:PI_SCRATCH_DIR) { $env:PI_SCRATCH_DIR } else { [IO.Path]::GetTempPath() }
 $OutputDirectory = Join-Path $root ('NxMonitor-overlay-' + [guid]::NewGuid().ToString('N'))
}
New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
if ($CapturePid -gt 0) { $PresentMon = (Resolve-Path -LiteralPath $PresentMon).Path }
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class OverlayProbe {
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int w, int height, uint flags);
 [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X; public int Y; }
 [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
 [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT p);
 [DllImport("user32.dll")] public static extern IntPtr GetWindowLongPtr(IntPtr h, int n);
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
 [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p);
 [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
 [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
}
'@
function Pump([int]$ms) { $end=[DateTime]::UtcNow.AddMilliseconds($ms); while([DateTime]::UtcNow -lt $end) { [System.Windows.Forms.Application]::DoEvents(); Start-Sleep -Milliseconds 15 } }
function Hotkey([byte]$key) {
 [OverlayProbe]::keybd_event(0x11,0,0,[UIntPtr]::Zero); [OverlayProbe]::keybd_event(0x12,0,0,[UIntPtr]::Zero); [OverlayProbe]::keybd_event($key,0,0,[UIntPtr]::Zero)
 [OverlayProbe]::keybd_event($key,0,2,[UIntPtr]::Zero); [OverlayProbe]::keybd_event(0x12,0,2,[UIntPtr]::Zero); [OverlayProbe]::keybd_event(0x11,0,2,[UIntPtr]::Zero)
}
function Click([int]$x,[int]$y) { [OverlayProbe]::SetCursorPos($x,$y)|Out-Null; [OverlayProbe]::mouse_event(2,0,0,0,[UIntPtr]::Zero); [OverlayProbe]::mouse_event(4,0,0,0,[UIntPtr]::Zero); Pump 300 }
$form=New-Object System.Windows.Forms.Form
$form.Text='NxMonitor isolated overlay test'; $form.StartPosition='Manual'; $form.Location=New-Object System.Drawing.Point(20,30); $form.Size=New-Object System.Drawing.Size(520,730); $form.BackColor=[System.Drawing.Color]::FromArgb(220,225,230)
$form.TopMost=$true
$script:clicks=0; $form.Add_MouseDown({$script:clicks++})
$old=New-Object OverlayProbe+POINT; [OverlayProbe]::GetCursorPos([ref]$old)|Out-Null
$p=$null
try {
 $form.Show(); $form.Activate(); Pump 400
 $err=Join-Path $OutputDirectory 'overlay-probe.err'; $out=Join-Path $OutputDirectory 'overlay-probe.out'
 $arguments=@('--window-only','--lang',$Language,'--style',$Style,'--smoke-seconds','20')
 if($CapturePid -gt 0) { $arguments=@('--presentmon',$PresentMon,'--pid',"$CapturePid",'--seconds','60','--lang',$Language,'--style',$Style,'--smoke-seconds','20') }
 $p=Start-Process -FilePath (Join-Path $project 'target\debug\nxmonitor-hud.exe') -WorkingDirectory $project -ArgumentList $arguments -RedirectStandardOutput $out -RedirectStandardError $err -PassThru
 Pump 3000
 $helper=Get-CimInstance Win32_Process -Filter "ParentProcessId=$($p.Id)" | Where-Object { $_.Name -like 'PresentMon*' }
 $session=$null
 if($helper -and $helper.CommandLine -match '--session_name\s+([^\s"]+)') { $session=$Matches[1] }
 $log=Get-Content $err -Raw
 if($log -match 'NATIVE_INIT_ERROR') { throw $log }
 if($log -notmatch 'hwnd=(\d+)') { throw "No HWND logged: $log" }
 $hwnd=[IntPtr]([long]$Matches[1]); $style=[OverlayProbe]::GetWindowLongPtr($hwnd,-20).ToInt64()
 $rect=New-Object OverlayProbe+RECT; [OverlayProbe]::GetWindowRect($hwnd,[ref]$rect)|Out-Null
 $form.BringToFront(); $form.Activate(); Pump 200
 [OverlayProbe]::SetWindowPos($form.Handle,[IntPtr](-1),0,0,0,0,0x13)|Out-Null
 [OverlayProbe]::SetWindowPos($hwnd,[IntPtr](-1),0,0,0,0,0x13)|Out-Null
 Pump 300
 $probe=New-Object OverlayProbe+POINT; $probe.X=$rect.Left+100; $probe.Y=$rect.Top+100
 if([OverlayProbe]::WindowFromPoint($probe) -ne $form.Handle) { throw 'Isolated background not directly under HUD; no alpha assertion or click sent' }
 $bitmap=New-Object System.Drawing.Bitmap(440,650)
 $graphics=[System.Drawing.Graphics]::FromImage($bitmap); $graphics.CopyFromScreen(20,30,0,0,$bitmap.Size)
 $bitmap.Save((Join-Path $OutputDirectory 'overlay-window.png')); $graphics.Dispose(); $bitmap.Dispose()
 $visible=New-Object System.Drawing.Bitmap(1,1)
 $g=[System.Drawing.Graphics]::FromImage($visible); $g.CopyFromScreen(($rect.Left+100),($rect.Top+100),0,0,$visible.Size)
 $pixel=$visible.GetPixel(0,0); $g.Dispose(); $visible.Dispose()
 "HUD_PIXEL R=$($pixel.R) G=$($pixel.G) B=$($pixel.B)"
 if($pixel.R -gt 150 -and $pixel.G -gt 150 -and $pixel.B -gt 150) { throw 'HUD invisible: isolated gray form shows through the supposed dark surface' }
 if($pixel.R -lt 15) { throw 'HUD Root is opaque: alpha blend not observed' }
 "HUD_RECT $($rect.Left),$($rect.Top),$($rect.Right),$($rect.Bottom)"
 $point=New-Object OverlayProbe+POINT; $point.X=$rect.Left+100; $point.Y=$rect.Top+150
 $under=[OverlayProbe]::WindowFromPoint($point)
 "PASS_THROUGH style=$style target=$under own=$hwnd form=$($form.Handle)"
 if(($style -band 0x20) -eq 0 -or $under -ne $form.Handle) { throw 'Pass-through did not resolve to the isolated form; no click sent' }
 Click $point.X $point.Y
 if($script:clicks -ne 1) { throw "Underlying test form did not receive click ($script:clicks)" }
 if($log -match 'edit=Ctrl\+Alt\+F(\d+)') { $edit=[byte](0x6f+[int]$Matches[1]) } else { throw 'Missing edit shortcut' }
 if($log -match 'quit=Ctrl\+Alt\+F(\d+)') { $quit=[byte](0x6f+[int]$Matches[1]) } else { throw 'Missing quit shortcut' }
 Hotkey $edit; Pump 800
 $style=[OverlayProbe]::GetWindowLongPtr($hwnd,-20).ToInt64(); $over=[OverlayProbe]::WindowFromPoint($point)
 "EDIT_MODE style=$style target=$over own=$hwnd"
 if(($style -band 0x20) -ne 0 -or $over -ne $hwnd) { throw 'Edit-mode hit test failed' }
 Click $point.X $point.Y
 if($script:clicks -ne 1) { throw 'Editing click leaked through HUD' }
 Hotkey $edit; Pump 600
 if(([OverlayProbe]::GetWindowLongPtr($hwnd,-20).ToInt64() -band 0x20) -eq 0) { throw 'Relock failed' }
 Hotkey $quit; Pump 1600; $p.Refresh()
 if(!$p.HasExited) { throw 'Global quit did not terminate HUD' }
 if($CapturePid -gt 0) {
   $remaining=Get-CimInstance Win32_Process -Filter "ParentProcessId=$($p.Id)" | Where-Object { $_.Name -like 'PresentMon*' }
   if($remaining) { throw 'Owned PresentMon helper survived HUD exit' }
   $finalLog=Get-Content $err -Raw
   if($finalLog -notmatch 'HUD_DROP') { throw 'No owned capture shutdown evidence' }
   if($finalLog -notmatch 'CAPTURE_SUMMARY received=([1-9]\d*) rejected=\d+ error=None') { throw 'No successful real target frame summary' }
   $sessions = (& logman query -ets) -join "`n"
   if($session -and $sessions.Contains($session)) { throw 'Owned ETW session survived HUD exit' }
   "CAPTURE_CLEANUP helper=$($helper.ProcessId) session=$session"
 }
 'PASS: topmost/pass-through/edit/relock/global-quit; underlying click count=1'
 Get-Content $err -Tail 15
 "EVIDENCE_DIRECTORY=$OutputDirectory"
} finally {
 if($p -and !$p.HasExited) { Stop-Process -Id $p.Id -ErrorAction SilentlyContinue }
 $form.Close(); $form.Dispose(); [OverlayProbe]::SetCursorPos($old.X,$old.Y)|Out-Null
}
