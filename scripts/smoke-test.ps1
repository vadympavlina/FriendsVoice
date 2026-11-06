$ErrorActionPreference = 'Stop'
$port = 45999
$password = 'smoke-test-password'
$binary = Join-Path $PSScriptRoot '..\target\x86_64-pc-windows-msvc\release\friends-voice-server.exe'
if (-not (Test-Path $binary)) { throw "Server executable not found: $binary" }

$server = $null
$client = $null
try {
    $server = Start-Process -FilePath $binary -ArgumentList @($password, "127.0.0.1:$port") -PassThru -WindowStyle Hidden
    $connected = $false
    for ($i = 0; $i -lt 40; $i++) {
        try {
            $client = [System.Net.Sockets.TcpClient]::new()
            $client.Connect('127.0.0.1', $port)
            $connected = $true
            break
        } catch {
            if ($client) { $client.Dispose(); $client = $null }
            Start-Sleep -Milliseconds 250
        }
    }
    if (-not $connected) { throw 'Server did not accept connections within 10 seconds' }
    $client.ReceiveTimeout = 3000
    $stream = $client.GetStream()
    $utf8 = [System.Text.UTF8Encoding]::new($false)
    $writer = [System.IO.StreamWriter]::new($stream, $utf8, 1024, $true)
    $reader = [System.IO.StreamReader]::new($stream, $utf8, $false, 1024, $true)
    $writer.AutoFlush = $true
    $writer.WriteLine('{"type":"hello","nickname":"Tester","password":"smoke-test-password"}')
    $response = $reader.ReadLine() | ConvertFrom-Json
    if ($response.type -ne 'welcome' -or $response.users -notcontains 'Tester') {
        throw 'Handshake failed: expected a welcome containing Tester'
    }
    $writer.WriteLine('{"type":"send_chat","text":"ping"}')
    $gotChat = $false
    for ($i = 0; $i -lt 6; $i++) {
        $packet = $reader.ReadLine() | ConvertFrom-Json
        if ($packet.type -eq 'chat' -and $packet.from -eq 'Tester' -and $packet.text -eq 'ping') {
            $gotChat = $true
            break
        }
    }
    if (-not $gotChat) { throw 'Chat broadcast failed' }
    Write-Host 'SMOKE TEST PASSED: server startup, handshake, chat message'
} finally {
    if ($client) { $client.Dispose() }
    if ($server -and -not $server.HasExited) {
        Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue
    }
}
