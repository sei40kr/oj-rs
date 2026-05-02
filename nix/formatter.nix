{
  inputs,
  perSystem,
  pkgs,
  ...
}:
let
  treefmtEval = import ./treefmt.nix { inherit inputs perSystem pkgs; };
in
treefmtEval.config.build.wrapper
