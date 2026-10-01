param([Parameter(Mandatory=$true)][string]$Ffmpeg,[Parameter(Mandatory=$true)][string]$Fpcalc,[ValidateSet('ARM64','x64')][string]$Architecture='ARM64')
$ErrorActionPreference='Stop'
$root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$target=if($Architecture -eq 'ARM64'){'aarch64-pc-windows-msvc'}else{'x86_64-pc-windows-msvc'}
$calculator=Join-Path $root "core\target\$target\release\examples\fingerprint.exe"
if(!(Test-Path $calculator)){throw 'Build the fingerprint example in the Visual Studio / LLVM environment first; see docs/FINGERPRINT.md.'}
$scratch=Join-Path ([IO.Path]::GetTempPath()) ('spiewak-fingerprint-reference-'+[guid]::NewGuid())
New-Item $scratch -ItemType Directory | Out-Null
try {
 $source=Join-Path $scratch 'source.wav'
 $filter='aevalsrc=0.25*sin(2*PI*220*pow(2\,mod(floor(t/0.4)\,12)/12)*t)+0.15*sin(2*PI*110*t)+0.1*sin(2*PI*660*t)*exp(-mod(t\,0.25)*20):s=11025:d=135'
 & $Ffmpeg -hide_banner -loglevel error -f lavfi -i $filter -c:a pcm_s16le $source
 if($LASTEXITCODE -ne 0){throw 'Reference source generation failed'}
 foreach($entry in @(@('wav16','pcm_s16le','wav',11025,1),@('wav24','pcm_s24le','wav',48000,2),@('flac','flac','flac',48000,2),@('mp3','libmp3lame','mp3',48000,2),@('aac','aac','m4a',48000,2),@('alac','alac','m4a',48000,2),@('ogg','libvorbis','ogg',48000,2))) {
  $file=Join-Path $scratch ($entry[0]+'.'+$entry[2])
  & $Ffmpeg -hide_banner -loglevel error -i $source -ar $entry[3] -ac $entry[4] -c:a $entry[1] $file
  if($LASTEXITCODE -ne 0){throw ('Fixture conversion failed: '+$entry[0])}
  $before=(Get-FileHash $file -Algorithm SHA256).Hash
  $reference=(& $Fpcalc -raw -json $file | ConvertFrom-Json)
  if($LASTEXITCODE -ne 0){throw 'Official fpcalc failed'}
  $actual=(& $calculator $file | ConvertFrom-Json)
  if($LASTEXITCODE -ne 0){throw 'Spiewak fingerprint failed'}
  if((Get-FileHash $file -Algorithm SHA256).Hash -ne $before){throw 'Fingerprinting changed source bytes'}
  if([Math]::Abs($reference.duration-$actual.duration) -gt .1 -or $reference.fingerprint.Count -ne $actual.raw.Count){throw 'Reference duration or item count differs'}
  [long]$bits=0
  for($i=0;$i -lt $actual.raw.Count;$i++){
   [long]$xor=([long]$reference.fingerprint[$i] -band 0xffffffffL) -bxor [long]$actual.raw[$i]
   while($xor -ne 0){$xor=$xor -band ($xor-1);$bits++}
  }
  $fraction=$bits/(32.0*$actual.raw.Count)
  # Lossy codecs can differ between FFmpeg and Symphonia; this is a compatibility check, not live identity evidence.
  if($fraction -gt .02){throw ('Fingerprint differs beyond the fixture decoder tolerance: '+$entry[0])}
  if($entry[0] -notin @('mp3','aac') -and $bits -ne 0){throw ('Lossless fingerprint differs: '+$entry[0])}
  Write-Output ('PASS '+$entry[0]+': '+$actual.raw.Count+' items; bit difference '+$fraction.ToString('P4')+'; exact source preserved')
 }
} finally { Remove-Item $scratch -Recurse -Force }
