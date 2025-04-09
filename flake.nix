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
    moxctl = {
      url = "github:unixpariah/moxctl";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      nixpkgs,
      rust-overlay,
      wgsl_analyzer,
      moxctl,
      ...
    }@inputs:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      overlays = [ (import rust-overlay) ];
      forAllSystems =
        function:
        nixpkgs.lib.genAttrs systems (
          system:
          let
            pkgs = import nixpkgs { inherit system overlays; };
          in
          function pkgs
        );
    in
    {
      devShells = forAllSystems (pkgs: {
        default =
          with pkgs;
          mkShell rec {
            buildInputs = [
              (rust-bin.selectLatestNightlyWith (
                toolchain:
                toolchain.default.override {
                  extensions = [
                    "rust-src"
                    "rustfmt"
                  ];
                }
              ))
              rust-analyzer-unwrapped
              nixd
              pkg-config
              lua5_4
              libxkbcommon
              vulkan-loader
              vulkan-headers
              vulkan-validation-layers
              wgsl_analyzer.packages.${system}.default
              wayland
              libpulseaudio
            ];
            LD_LIBRARY_PATH = "${lib.makeLibraryPath buildInputs}";
          };
      });

      packages = forAllSystems (pkgs: {
        default = pkgs.callPackage ./nix/package.nix { moxctl = moxctl.packages.${pkgs.system}.default; };
      });

      homeManagerModules = {
        default = import ./nix/home-manager.nix;
        stylix = import ./nix/stylix.nix;
      };
    };
}
