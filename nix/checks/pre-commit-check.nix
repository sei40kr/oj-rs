{ inputs, pkgs, ... }:
let
  treefmtEval = inputs.treefmt.lib.evalModule pkgs ../treefmt.nix;
in
inputs.git-hooks.lib.${pkgs.stdenv.hostPlatform.system}.run {
  src = inputs.self;
  settings.rust.check.cargoDeps = pkgs.rustPlatform.importCargoLock {
    lockFile = ../../Cargo.lock;
  };
  hooks = {
    nil.enable = true;
    statix.enable = true;
    clippy = {
      enable = true;
      settings.denyWarnings = true;
    };
    treefmt = {
      enable = true;
      package = treefmtEval.config.build.wrapper;
    };
  };
}
