<#
.SYNOPSIS
Builds and signs the Phira Android APK.

.DESCRIPTION
Debug builds use a generated debug key. Release builds never fall back to that
key: provide -ReleaseKeyStore and -ReleaseKeyAlias (or set
PHIRA_ANDROID_KEYSTORE and PHIRA_ANDROID_KEY_ALIAS), plus
PHIRA_ANDROID_KEYSTORE_PASSWORD. PHIRA_ANDROID_KEY_PASSWORD is optional and
defaults to the keystore password.

VersionName defaults to the phira package version from Cargo metadata.
VersionCode defaults to major * 1,000,000 + minor * 1,000 + patch, and both can
be overridden explicitly.
#>
[CmdletBinding()]
param(
    [string]$SdkRoot = $env:ANDROID_SDK_ROOT,
    [string]$NdkRoot = $env:ANDROID_NDK_ROOT,
    [string]$BuildToolsVersion = "35.0.0",
    [int]$CompileSdkVersion = 35,
    [int]$MinSdkVersion = 23,
    [int]$VersionCode = 0,
    [string]$VersionName,
    [string]$ReleaseKeyStore = $env:PHIRA_ANDROID_KEYSTORE,
    [string]$ReleaseKeyAlias = $env:PHIRA_ANDROID_KEY_ALIAS,
    [switch]$Release
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Assert-LastExitCode([string]$Description) {
    if ($LASTEXITCODE -ne 0) {
        throw "$Description failed with exit code $LASTEXITCODE"
    }
}

function Require-File([string]$Path, [string]$Description) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "$Description was not found at '$Path'"
    }
    return (Resolve-Path -LiteralPath $Path).Path
}

function Initialize-MsvcEnvironment {
    $HostTriple = (& rustc.exe -vV | Select-String '^host:\s+(.+)$').Matches.Groups[1].Value
    Assert-LastExitCode "Rust host query"
    if (-not $HostTriple.EndsWith('-pc-windows-msvc') -or (Get-Command link.exe -ErrorAction SilentlyContinue)) {
        return
    }

    $VsWhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path -LiteralPath $VsWhere -PathType Leaf) {
        $Installation = & $VsWhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        if ($LASTEXITCODE -eq 0 -and -not [string]::IsNullOrWhiteSpace($Installation)) {
            $DevCmd = Join-Path $Installation "Common7\Tools\VsDevCmd.bat"
            if (Test-Path -LiteralPath $DevCmd -PathType Leaf) {
                $Command = "`"$DevCmd`" -no_logo -arch=x64 -host_arch=x64 >nul && set"
                foreach ($Line in & $env:ComSpec /d /s /c $Command) {
                    if ($Line -match '^([^=]+)=(.*)$') {
                        Set-Item -Path "Env:$($Matches[1])" -Value $Matches[2]
                    }
                }
            }
        }
    }

    if (-not (Get-Command link.exe -ErrorAction SilentlyContinue)) {
        throw "The Rust MSVC host linker 'link.exe' is unavailable. Install Visual Studio 2022 Build Tools with the Desktop development with C++ workload, or run this script from a Developer PowerShell."
    }
}

function Resolve-VersionCode([string]$Version) {
    if ($Version -notmatch '^(\d+)\.(\d+)\.(\d+)') {
        throw "Cannot derive an Android version code from Cargo version '$Version'. Pass -VersionCode explicitly."
    }
    $Code = [int64]$Matches[1] * 1000000 + [int64]$Matches[2] * 1000 + [int64]$Matches[3]
    if ($Code -lt 1 -or $Code -gt [int]::MaxValue) {
        throw "Derived Android version code '$Code' is outside the supported range. Pass -VersionCode explicitly."
    }
    return [int]$Code
}

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

if ([string]::IsNullOrWhiteSpace($SdkRoot)) {
    $SdkRoot = $env:ANDROID_HOME
}
if ([string]::IsNullOrWhiteSpace($SdkRoot)) {
    $defaultSdk = Join-Path $env:LOCALAPPDATA "Android\Sdk"
    if (Test-Path -LiteralPath $defaultSdk -PathType Container) {
        $SdkRoot = $defaultSdk
    }
}
if ([string]::IsNullOrWhiteSpace($SdkRoot) -or -not (Test-Path -LiteralPath $SdkRoot -PathType Container)) {
    throw "Set ANDROID_SDK_ROOT (or ANDROID_HOME), or pass -SdkRoot."
}
$SdkRoot = (Resolve-Path -LiteralPath $SdkRoot).Path

if ([string]::IsNullOrWhiteSpace($NdkRoot)) {
    $ndkParent = Join-Path $SdkRoot "ndk"
    if (Test-Path -LiteralPath $ndkParent -PathType Container) {
        $NdkRoot = Get-ChildItem -LiteralPath $ndkParent -Directory |
            Sort-Object { try { [version]$_.Name } catch { [version]"0.0" } } -Descending |
            Select-Object -First 1 -ExpandProperty FullName
    }
}
if ([string]::IsNullOrWhiteSpace($NdkRoot) -or -not (Test-Path -LiteralPath $NdkRoot -PathType Container)) {
    throw "Set ANDROID_NDK_ROOT, install an SDK-side NDK, or pass -NdkRoot."
}
$NdkRoot = (Resolve-Path -LiteralPath $NdkRoot).Path

$BuildTools = Join-Path $SdkRoot "build-tools\$BuildToolsVersion"
$AndroidJar = Require-File (Join-Path $SdkRoot "platforms\android-$CompileSdkVersion\android.jar") "Android $CompileSdkVersion platform JAR"
$Aapt = Require-File (Join-Path $BuildTools "aapt.exe") "aapt"
$Aapt2 = Require-File (Join-Path $BuildTools "aapt2.exe") "aapt2"
$D8 = Require-File (Join-Path $BuildTools "d8.bat") "d8"
$ZipAlign = Require-File (Join-Path $BuildTools "zipalign.exe") "zipalign"
$ApkSigner = Require-File (Join-Path $BuildTools "apksigner.bat") "apksigner"

$NdkBin = Join-Path $NdkRoot "toolchains\llvm\prebuilt\windows-x86_64\bin"
$AndroidLinker = Require-File (Join-Path $NdkBin "aarch64-linux-android$MinSdkVersion-clang.cmd") "Android C linker"
$Cxx = Require-File (Join-Path $NdkBin "aarch64-linux-android$MinSdkVersion-clang++.cmd") "Android C++ linker"
$Ar = Require-File (Join-Path $NdkBin "llvm-ar.exe") "Android archiver"
$CppShared = Require-File (
    Join-Path $NdkRoot "toolchains\llvm\prebuilt\windows-x86_64\sysroot\usr\lib\aarch64-linux-android\libc++_shared.so"
) "Android C++ runtime"

$JavaHome = $env:JAVA_HOME
if ([string]::IsNullOrWhiteSpace($JavaHome)) {
    throw "Set JAVA_HOME to a JDK (Java 17 is supported)."
}
$Javac = Require-File (Join-Path $JavaHome "bin\javac.exe") "javac"
$Jar = Require-File (Join-Path $JavaHome "bin\jar.exe") "jar"
$KeyTool = Require-File (Join-Path $JavaHome "bin\keytool.exe") "keytool"
$Cargo = (Get-Command cargo.exe -ErrorAction Stop).Source
Initialize-MsvcEnvironment

$TargetTriple = "aarch64-linux-android"
$CargoProfile = if ($Release) { "release" } else { "debug" }
$MetadataJson = & $Cargo metadata --format-version 1 --filter-platform $TargetTriple
Assert-LastExitCode "Cargo metadata query"
$Metadata = $MetadataJson | ConvertFrom-Json
$PhiraPackage = @($Metadata.packages | Where-Object name -eq "phira")
if ($PhiraPackage.Count -ne 1) {
    throw "Expected exactly one phira package, found $($PhiraPackage.Count)."
}
if ([string]::IsNullOrWhiteSpace($VersionName)) {
    $VersionName = $PhiraPackage[0].version
}
if ($VersionCode -eq 0) {
    $VersionCode = Resolve-VersionCode $VersionName
}
if ($VersionCode -lt 1) {
    throw "Android version code must be positive."
}

if ($Release) {
    if ([string]::IsNullOrWhiteSpace($ReleaseKeyStore)) {
        throw "Set PHIRA_ANDROID_KEYSTORE or pass -ReleaseKeyStore for a release build."
    }
    if ([string]::IsNullOrWhiteSpace($ReleaseKeyAlias)) {
        throw "Set PHIRA_ANDROID_KEY_ALIAS or pass -ReleaseKeyAlias for a release build."
    }
    if ([string]::IsNullOrWhiteSpace($env:PHIRA_ANDROID_KEYSTORE_PASSWORD)) {
        throw "Set PHIRA_ANDROID_KEYSTORE_PASSWORD for a release build."
    }
    $ReleaseKeyStore = Require-File $ReleaseKeyStore "release keystore"
}

$ArtifactRoot = [IO.Path]::GetFullPath((Join-Path $RepoRoot "target\android-artifacts"))
$RepoPrefix = $RepoRoot.TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
if (-not $ArtifactRoot.StartsWith($RepoPrefix, [StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to use an artifact directory outside the repository: '$ArtifactRoot'"
}
$Stage = Join-Path $ArtifactRoot "$CargoProfile\staging"
if (Test-Path -LiteralPath $Stage) {
    Remove-Item -LiteralPath $Stage -Recurse -Force
}
$JavaSource = Join-Path $Stage "java"
$Classes = Join-Path $Stage "classes"
$Dex = Join-Path $Stage "dex"
$Native = Join-Path $Stage "lib\arm64-v8a"
$Resources = Join-Path $Stage "res"
$ApkDirectory = Join-Path $ArtifactRoot "$CargoProfile\apk"
New-Item -ItemType Directory -Force -Path $JavaSource, $Classes, $Dex, $Native, $Resources, $ApkDirectory | Out-Null

$env:ANDROID_NDK_ROOT = $NdkRoot
$env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = $AndroidLinker
$env:CC_aarch64_linux_android = $AndroidLinker
$env:CXX_aarch64_linux_android = $Cxx
$env:AR_aarch64_linux_android = $Ar
$env:CXXSTDLIB = "c++"

$CargoArgs = @("build", "-p", "phira", "--target", $TargetTriple, "--no-default-features")
if ($Release) {
    $CargoArgs += "--release"
}
& $Cargo @CargoArgs
Assert-LastExitCode "Rust Android build"

$RustLibrary = Require-File (
    Join-Path $RepoRoot "target\$TargetTriple\$CargoProfile\libphira.so"
) "Phira Android library"
Copy-Item -LiteralPath $RustLibrary -Destination (Join-Path $Native "libphira.so")
Copy-Item -LiteralPath $CppShared -Destination (Join-Path $Native "libc++_shared.so")

$MainTemplate = Require-File (Join-Path $RepoRoot "vendor\prpr-miniquad\java\MainActivity.java") "miniquad MainActivity template"
$QuadNativeTemplate = Require-File (Join-Path $RepoRoot "vendor\prpr-miniquad\java\QuadNative.java") "miniquad JNI declarations"
$MainJava = Join-Path $JavaSource "MainActivity.java"
$QuadNativeJava = Join-Path $JavaSource "QuadNative.java"
$MainText = [IO.File]::ReadAllText($MainTemplate).Replace("TARGET_PACKAGE_NAME", "org.flos.phira").Replace("LIBRARY_NAME", "phira")
$QuadNativeText = [IO.File]::ReadAllText($QuadNativeTemplate).Replace("LIBRARY_NAME", "phira")
[IO.File]::WriteAllText($MainJava, $MainText, [Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText($QuadNativeJava, $QuadNativeText, [Text.UTF8Encoding]::new($false))

$ProjectJava = Get-ChildItem -LiteralPath (Join-Path $RepoRoot "android\java") -Recurse -Filter "*.java" | Select-Object -ExpandProperty FullName
$JavaFiles = @($MainJava, $QuadNativeJava) + @($ProjectJava)
& $Javac --release 8 -encoding UTF-8 -classpath $AndroidJar -d $Classes $JavaFiles
Assert-LastExitCode "Android Java compilation"

$ClassesJar = Join-Path $Stage "classes.jar"
Push-Location $Classes
try {
    & $Jar cf $ClassesJar .
    Assert-LastExitCode "Java archive creation"
} finally {
    Pop-Location
}

$RustlsAndroid = @($Metadata.packages | Where-Object name -eq "rustls-platform-verifier-android")
if ($RustlsAndroid.Count -ne 1) {
    throw "Expected exactly one rustls-platform-verifier-android package, found $($RustlsAndroid.Count)."
}
$RustlsRoot = Split-Path -Parent $RustlsAndroid[0].manifest_path
$RustlsAar = @(Get-ChildItem -LiteralPath (Join-Path $RustlsRoot "maven") -Recurse -Filter "*.aar")
if ($RustlsAar.Count -ne 1) {
    throw "Expected exactly one rustls Android AAR, found $($RustlsAar.Count)."
}
$RustlsSupport = Join-Path $Stage "rustls-support"
New-Item -ItemType Directory -Force -Path $RustlsSupport | Out-Null
Push-Location $RustlsSupport
try {
    & $Jar xf $RustlsAar[0].FullName "classes.jar"
    Assert-LastExitCode "rustls Android AAR extraction"
} finally {
    Pop-Location
}
$RustlsClassesJar = Require-File (Join-Path $RustlsSupport "classes.jar") "rustls Android classes"

$KotlinVersion = "1.6.10"
$KotlinSha256 = "5305F7A4DEE7A6CB79A29C258ACA93DE47B49588A6DFC6DA01BD8772589EA66C"
$DependencyCache = Join-Path $ArtifactRoot "dependencies"
New-Item -ItemType Directory -Force -Path $DependencyCache | Out-Null
$KotlinJar = Join-Path $DependencyCache "kotlin-stdlib-$KotlinVersion.jar"
if (-not (Test-Path -LiteralPath $KotlinJar -PathType Leaf) -or (Get-FileHash -LiteralPath $KotlinJar -Algorithm SHA256).Hash -ne $KotlinSha256) {
    Invoke-WebRequest -Uri "https://repo.maven.apache.org/maven2/org/jetbrains/kotlin/kotlin-stdlib/$KotlinVersion/kotlin-stdlib-$KotlinVersion.jar" -OutFile $KotlinJar
}
if ((Get-FileHash -LiteralPath $KotlinJar -Algorithm SHA256).Hash -ne $KotlinSha256) {
    throw "Downloaded Kotlin standard library failed SHA-256 verification."
}

& $D8 --min-api $MinSdkVersion --lib $AndroidJar --output $Dex $ClassesJar $RustlsClassesJar $KotlinJar
Assert-LastExitCode "DEX compilation"

$Manifest = Require-File (Join-Path $RepoRoot "android\AndroidManifest.xml") "Android manifest"
$Unsigned = Join-Path $Stage "phira-unsigned.apk"
$Aligned = Join-Path $Stage "phira-aligned.apk"
Copy-Item -LiteralPath (Join-Path $RepoRoot "android\res\values") -Destination $Resources -Recurse
$Mipmap = Join-Path $Resources "mipmap"
New-Item -ItemType Directory -Force -Path $Mipmap | Out-Null
Copy-Item -LiteralPath (Join-Path $RepoRoot "assets\icon.png") -Destination (Join-Path $Mipmap "icon.png")
$CompiledResources = Join-Path $Stage "resources.zip"
& $Aapt2 compile --dir $Resources -o $CompiledResources
Assert-LastExitCode "Android resource compilation"
& $Aapt2 link -o $Unsigned -I $AndroidJar --manifest $Manifest --min-sdk-version $MinSdkVersion --target-sdk-version $CompileSdkVersion --version-code $VersionCode --version-name $VersionName -A (Join-Path $RepoRoot "assets") $CompiledResources
Assert-LastExitCode "Android resource packaging"

Push-Location $Dex
try {
    & $Aapt add $Unsigned "classes.dex"
    Assert-LastExitCode "DEX packaging"
} finally {
    Pop-Location
}
Push-Location $Stage
try {
    & $Aapt add $Unsigned "lib/arm64-v8a/libphira.so" "lib/arm64-v8a/libc++_shared.so"
    Assert-LastExitCode "Native library packaging"
} finally {
    Pop-Location
}

& $ZipAlign -f -p 4 $Unsigned $Aligned
Assert-LastExitCode "APK alignment"

$FinalApk = Join-Path $ApkDirectory "phira.apk"
if ($Release) {
    $KeyPasswordSource = if ([string]::IsNullOrWhiteSpace($env:PHIRA_ANDROID_KEY_PASSWORD)) {
        "env:PHIRA_ANDROID_KEYSTORE_PASSWORD"
    } else {
        "env:PHIRA_ANDROID_KEY_PASSWORD"
    }
    & $ApkSigner sign --ks $ReleaseKeyStore --ks-key-alias $ReleaseKeyAlias --ks-pass env:PHIRA_ANDROID_KEYSTORE_PASSWORD --key-pass $KeyPasswordSource --out $FinalApk $Aligned
} else {
    $KeyStore = Join-Path $ArtifactRoot "debug.keystore"
    if (-not (Test-Path -LiteralPath $KeyStore -PathType Leaf)) {
        & $KeyTool -genkeypair -keystore $KeyStore -storepass android -alias androiddebugkey -keypass android -keyalg RSA -keysize 2048 -validity 10000 -dname "CN=Android Debug,O=Android,C=US"
        Assert-LastExitCode "Debug keystore creation"
    }
    & $ApkSigner sign --ks $KeyStore --ks-key-alias androiddebugkey --ks-pass pass:android --key-pass pass:android --out $FinalApk $Aligned
}
Assert-LastExitCode "APK signing"
& $ApkSigner verify --verbose $FinalApk
Assert-LastExitCode "APK signature verification"

Write-Output "Built $FinalApk"
