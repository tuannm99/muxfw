if vim.g.loaded_muxwf_nvim == 1 then
  return
end
vim.g.loaded_muxwf_nvim = 1

local muxwf = require("muxwf")
muxwf.setup()
_G.muxwf_nvim = muxwf
