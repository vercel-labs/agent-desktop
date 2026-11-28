#Requires -Version 5.1
# Compiles the two scratch targets with the in-box .NET Framework compiler (C# 5).
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $PSCommandPath
$framework = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319'
$csc = Join-Path $framework 'csc.exe'
$wpf = Join-Path $framework 'WPF'
$bin = Join-Path $here 'bin'
New-Item -ItemType Directory -Force $bin | Out-Null
& $csc /nologo /target:winexe /langversion:5 "/out:$bin\SfpForms.exe" /reference:System.dll /reference:System.Drawing.dll /reference:System.Windows.Forms.dll "$here\SfpForms.cs"
if ($LASTEXITCODE -ne 0) { throw 'SfpForms build failed' }
& $csc /nologo /target:winexe /langversion:5 "/out:$bin\SfpWpf.exe" /reference:System.dll /reference:System.Core.dll /reference:System.Xaml.dll "/reference:$wpf\PresentationFramework.dll" "/reference:$wpf\PresentationCore.dll" "/reference:$wpf\WindowsBase.dll" "$here\SfpWpf.cs"
if ($LASTEXITCODE -ne 0) { throw 'SfpWpf build failed' }
Get-ChildItem $bin -Filter *.exe | ForEach-Object { 'built ' + $_.Name + ' ' + $_.Length }
