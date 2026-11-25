---
"@nimblepost/desktop": patch
---

On Linux, disable WebKit DMA-BUF when the active display uses software rendering. This reduces startup time and idle memory on Debian x64 with LLVMpipe while keeping the JavaScript JIT enabled.
