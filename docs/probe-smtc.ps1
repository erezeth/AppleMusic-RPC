using namespace System.Runtime.InteropServices.WindowsRuntime

# Dumps every Windows media session SMTC can see. Read-only.
# Use to verify Apple Music exposes metadata before trusting the app.
Add-Type -AssemblyName System.Runtime.WindowsRuntime
$asTask = ([System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {
  $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and
  $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1'
})[0]

function Await($op, $type) {
  $task = $asTask.MakeGenericMethod($type).Invoke($null, @($op))
  $task.Wait(10000) | Out-Null
  $task.Result
}

$managerType = [Windows.Media.Control.GlobalSystemMediaTransportControlsSessionManager, Windows.Media.Control, ContentType = WindowsRuntime]
$propsType = [Windows.Media.Control.GlobalSystemMediaTransportControlsSessionMediaProperties, Windows.Media.Control, ContentType = WindowsRuntime]

$manager = Await ($managerType::RequestAsync()) $managerType
$sessions = $manager.GetSessions()
"Apple Music sessions found: $($sessions.Count)"
foreach ($session in $sessions) {
  $props = Await ($session.TryGetMediaPropertiesAsync()) $propsType
  $timeline = $session.GetTimelineProperties()
  $status = $session.GetPlaybackInfo().PlaybackStatus
  [pscustomobject]@{
    Aumid    = $session.SourceAppUserModelId
    Status   = $status
    Title    = $props.Title
    Artist   = $props.Artist
    Album    = $props.AlbumTitle
    Artwork  = ($null -ne $props.Thumbnail)
    Position = $timeline.Position
    End      = $timeline.EndTime
  } | Format-List
}
