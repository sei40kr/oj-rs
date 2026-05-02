{
  inputs,
  perSystem,
  pkgs,
}:
inputs.treefmt.lib.evalModule pkgs {
  projectRootFile = "flake.nix";

  programs.nixfmt.enable = true;
  programs.rustfmt = {
    enable = true;
    package = perSystem.fenix.stable.rustfmt;
  };
}
