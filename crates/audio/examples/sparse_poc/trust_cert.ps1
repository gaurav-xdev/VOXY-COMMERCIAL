$cert = Get-Item "Cert:\CurrentUser\My\27FA812177B4BB9432133DBF599B7B8A33B8915A"
$store = New-Object System.Security.Cryptography.X509Certificates.X509Store("Root", "CurrentUser")
$store.Open([System.Security.Cryptography.X509Certificates.OpenFlags]::ReadWrite)
$store.Add($cert)
$store.Close()
Write-Output "Successfully added cert to CurrentUser\Root"
