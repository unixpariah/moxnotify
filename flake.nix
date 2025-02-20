{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    wgsl_analyzer = {
      url = "github:wgsl-analyzer/wgsl-analyzer";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { nixpkgs, ... }@inputs:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems =
        function: nixpkgs.lib.genAttrs systems (system: function nixpkgs.legacyPackages.${system});
    in
    {
      overlays.default = final: prev: {
        rustToolchain = prev.rust-bin.rust.fromRustupToolchainFile ./rust-toolchain.toml;
      };

      devShells = forAllSystems (pkgs: {
        default =
          with pkgs;
          mkShell rec {
            buildInputs = [
              rust-analyzer
              cargo
              rustfmt
              clippy
              nixd
              pkg-config
              lua5_4
              libxkbcommon
              vulkan-loader
              vulkan-headers
              vulkan-validation-layers
              inputs.wgsl_analyzer.packages.${system}.default
              wayland
            ];
            LD_LIBRARY_PATH = "${lib.makeLibraryPath buildInputs}";
          };
      });

      packages = forAllSystems (pkgs: {
        default = pkgs.callPackage ./nix/package.nix { };
      });

      homeManagerModules = {
        default = import ./nix/home-manager.nix;
        stylix = import ./nix/stylix.nix;
      };
    };
}
