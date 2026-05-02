{
  inputs,
  perSystem,
  pkgs,
  ...
}:
let
  treefmtEval = import ../treefmt.nix { inherit inputs perSystem pkgs; };
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
      packageOverrides = {
        inherit (perSystem.fenix.stable) cargo clippy;
      };
    };
    treefmt = {
      enable = true;
      package = treefmtEval.config.build.wrapper;
    };
  };
}
