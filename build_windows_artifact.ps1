# PowerShell script to build the Walksnail OSD Tool Windows artifact locally.

# 1. Clean up old artifacts
if (Test-Path "target/wix") { Remove-Item -Recurse -Force "target/wix" }

# 2. Extract ffmpeg/ffprobe if not already extracted
# Note: You need a tool like 7-Zip or WinRAR to extract this manually if this step fails.
$ffmpeg_7z = "ext/ffmpeg/windows64/ffmpeg-6.0-essentials_build.7z"
$ffmpeg_dir = "ext/ffmpeg/windows64/ffmpeg-6.0-essentials_build"

if (!(Test-Path $ffmpeg_dir)) {
    Write-Host "Please extract '$ffmpeg_7z' to '$ffmpeg_dir' manually before continuing." -ForegroundColor Yellow
    # You can try using tar if your Windows version supports it:
    # tar -xf $ffmpeg_7z -C ext/ffmpeg/windows64/
}

# 3. Build the binary with the windows-installer feature
Write-Host "Building project..." -ForegroundColor Cyan
cargo build --release --features windows-installer

# 4. Create the MSI installer using cargo-wix
Write-Host "Creating MSI installer..." -ForegroundColor Cyan
cargo wix --package walksnail-osd-tool --include _deploy\windows\wix\main.wxs --nocapture --no-build

# 5. Create the final zip
$msi = Get-ChildItem "target/wix/*.msi" | Select-Object -First 1
if ($msi) {
    Write-Host "Zipping installer: $($msi.Name)" -ForegroundColor Green
    if (!(Test-Path "_deploy")) { New-Item -ItemType Directory "_deploy" }
    Compress-Archive -Path $msi.FullName -DestinationPath "_deploy/walksnail-osd-tool-windows.zip" -Force
    Write-Host "Artifact created: _deploy/walksnail-osd-tool-windows.zip" -ForegroundColor Green
} else {
    Write-Error "Failed to find the generated MSI file."
}
