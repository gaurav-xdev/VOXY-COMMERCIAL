$cert = New-SelfSignedCertificate -Type Custom -Subject "CN=VoxyDevTestCert" -KeyUsage DigitalSignature -FriendlyName "VOXY Test Cert" -CertStoreLocation "Cert:\CurrentUser\My" -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3", "2.5.29.19={text}")
Write-Output "Cert Subject: $($cert.Subject)"
Write-Output "Cert Thumbprint: $($cert.Thumbprint)"

# Sign the MSIX package
$signtool = "C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\signtool.exe"
$msix = "$PSScriptRoot\SparseMicPoc.msix"
& $signtool sign /fd SHA256 /sha1 $cert.Thumbprint /s My /sm $msix
if ($LASTEXITCODE -ne 0) {
    & $signtool sign /fd SHA256 /sha1 $cert.Thumbprint $msix
}
