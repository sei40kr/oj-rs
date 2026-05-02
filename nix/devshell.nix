{
  inputs,
  perSystem,
  pkgs,
  ...
}:
let
  pre-commit-check = import ./checks/pre-commit-check.nix { inherit inputs perSystem pkgs; };
  rustToolchain = perSystem.fenix.combine (
    with perSystem.fenix.stable;
    [
      cargo
      clippy
      rustc
      rustfmt
      rust-src
    ]
  );
in
pkgs.mkShell {
  packages = [
    rustToolchain
    pkgs.mold
    pkgs.sccache
  ];

  env = {
    RUSTC_WRAPPER = "sccache";
    RUSTFLAGS = "-C link-arg=-fuse-ld=mold";
  };

  shellHook = ''
    ${pre-commit-check.shellHook}

    # Load custom bash code
  '';
}
