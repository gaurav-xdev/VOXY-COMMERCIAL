# REA Installation Verification

## 1. Installation Overview
- **Package**: `rea-agents@6.8.0`
- **Installation Method**: Global npm installation (`npm install --global rea-agents@6.8.0`)
- **Execution Date**: 2026-10-25
- **Host OS**: Windows 11 Home Single Language (10.0.26200, 64-bit)
- **Node Runtime**: Node.js v24.14.0 (npm v11.9.0)

## 2. Verification Command Outputs

### Version Check
```powershell
$ rea --version
6.8.0
```

### Binary Path Resolution
- Executable shim located at: `C:\nvm4w\nodejs\rea` and `C:\nvm4w\nodejs\rea.cmd`
- Global node_modules located at: `C:\Users\gaurav\AppData\Roaming\npm\node_modules\rea-agents`

### Health Check (`rea doctor`)
- **CLI Package Version**: `6.8.0`
- **SDK Components**: `server: 2.3.1`, `client_test: 2.3.1`, `core: 2.3.1`
- **Catalog Counts**:
  - CLI commands: 175
  - MCP tools: 229
  - MCP prompts: 6
- **Digests**:
  - `tools_sha256`: `a98940ba69acd9c1781d180cee362d4ddab18f77eefdd3e3216f10e1bf98e5e9`
  - `prompts_sha256`: `9609379442110570cd52fdea61a777242350873bc28958f54827ca3590e680b7`
  - `combined_sha256`: `fe938a7c6eba3763b22ae2022de76dc4a7812d35d86f1d4efd36b9962b04da0b`

### External Provider Audit
- External decompiler engines (Hopper, Ghidra, IDA, JADX, Kuna) are unconfigured / not installed in this environment.
- Core static AST, file offset, bundle analysis, and CLI commands operate natively without external GUI decompilers.

## 3. Status
Verified operational as a CLI binary and analysis framework.
