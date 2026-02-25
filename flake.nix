{
  description = "noctoggle - automatic topbar toggle for Noctalia Shell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    git-hooks-nix = {
      url = "github:cachix/git-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs: inputs.flake-parts.lib.mkFlake { inherit inputs; } {
    imports = [
      inputs.treefmt-nix.flakeModule
      inputs.git-hooks-nix.flakeModule
    ];

    systems = [ "x86_64-linux" "aarch64-linux" ];

    perSystem = { config, self', pkgs, ... }: {
      treefmt = {
        programs = {
          nixpkgs-fmt.enable = true;
          rustfmt.enable = true;
          deadnix.enable = true;
          statix.enable = true;
        };
      };

      pre-commit.settings = {
        settings = {
          rust = {
            cargoManifestPath = "./Cargo.toml";
            check.cargoDeps = pkgs.rustPlatform.importCargoLock { lockFile = ./Cargo.lock; };
          };
        };
        hooks = {
          treefmt.enable = true;
          clippy.enable = true;
        };
      };

      packages = rec {
        noctoggle = pkgs.rustPlatform.buildRustPackage {
          pname = "noctoggle";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          meta.mainProgram = "noctoggle";
        };
        default = noctoggle;
      };

      devShells.default = pkgs.mkShell {
        inputsFrom = [ self'.packages.default ];
        nativeBuildInputs = with pkgs; [
          rust-analyzer
          clippy
        ];
        shellHook = config.pre-commit.installationScript;
      };
    };

    flake =
      let
        inherit (inputs.nixpkgs) lib;
      in
      {
        nixosModules = rec {
          noctoggle = { pkgs, ... }: {
            imports = [ ./nix/module.nix ];

            options.services.noctoggle.package = lib.mkOption {
              type = lib.types.package;
              description = "The noctoggle package to use.";
              default = inputs.self.packages.${pkgs.stdenv.hostPlatform.system}.noctoggle;
            };
          };
          default = noctoggle;
        };
      };
  };
}
