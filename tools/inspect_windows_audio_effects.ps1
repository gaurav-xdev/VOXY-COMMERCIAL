# Inspect Windows Audio Effects and APO configuration for Intel SST Microphone
Write-Host "=== WINDOWS AUDIO DRIVER / APO REGISTRY INSPECTION ==="

$renderKeys = Get-ChildItem -Path "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Capture" -ErrorAction SilentlyContinue
foreach ($k in $renderKeys) {
    $props = Get-ItemProperty -Path "$($k.PSPath)\Properties" -ErrorAction SilentlyContinue
    # PKEY_Device_FriendlyName {a45c254e-df1c-4efd-8020-67d146a850e0},2
    $friendlyName = $props.'{a45c254e-df1c-4efd-8020-67d146a850e0},2'
    if ($friendlyName -like "*Intel*" -or $friendlyName -like "*Microphone*") {
        Write-Host "`nCapture Device: $friendlyName"
        Write-Host "ID: $($k.PSChildName)"
        
        # FX Properties:
        # PKEY_FX_PreMixEffectClsid: {d04e05a6-594b-4fb6-a80d-01af5eed7d1d},1
        # PKEY_FX_PostMixEffectClsid: {d04e05a6-594b-4fb6-a80d-01af5eed7d1d},2
        # PKEY_AudioEndpoint_ControlPanelPageProvider: {3a715d31-f0a1-4c4c-89ee-3e96ac3fa6d7},1
        foreach ($prop in $props.PSObject.Properties) {
            if ($prop.Name -like "*{d04e05a6*" -or $prop.Name -like "*Mode*" -or $prop.Name -like "*{624f3449*") {
                Write-Host "  $($prop.Name) = $($prop.Value)"
            }
        }
        
        # Check FxProperties subkey
        $fxPath = "$($k.PSPath)\FxProperties"
        if (Test-Path $fxPath) {
            Write-Host "  [FxProperties Found]"
            $fxProps = Get-ItemProperty -Path $fxPath
            foreach ($fp in $fxProps.PSObject.Properties) {
                if ($fp.Name -notlike "PS*") {
                    Write-Host "    $($fp.Name) = $($fp.Value)"
                }
            }
        }
    }
}
