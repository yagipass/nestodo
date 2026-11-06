{
  description = "A family todo list on Cloudflare Workers with D1 storage, built with Void and protected by Cloudflare Access";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    git-hooks = {
      url = "github:cachix/git-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [
        inputs.treefmt-nix.flakeModule
        inputs.git-hooks.flakeModule
      ];

      systems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-linux"
      ];

      perSystem =
        {
          config,
          lib,
          pkgs,
          system,
          ...
        }:
        let
          migrations = "^apps/web/db/migrations/";
          androidSdk =
            (pkgs.androidenv.composeAndroidPackages {
              platformVersions = [ "37.0" ];
              buildToolsVersions = [ "36.0.0" ];
              toolsVersion = null;
              includeCmake = false;
              includeEmulator = pkgs.stdenv.hostPlatform.isDarwin;
              includeSystemImages = pkgs.stdenv.hostPlatform.isDarwin;
              systemImageTypes = [ "google_apis" ];
              abiVersions = [ "arm64-v8a" ];
            }).androidsdk;
          jdk = pkgs.jdk25;
        in
        {
          _module.args.pkgs = import inputs.nixpkgs {
            inherit system;
            config = {
              allowUnfree = true;
              android_sdk.accept_license = true;
            };
          };

          treefmt = {
            projectRootFile = "flake.nix";
            flakeCheck = false;
            programs.nixfmt.enable = true;
            programs.rustfmt.enable = true;
          };

          pre-commit.settings.hooks = {
            treefmt.enable = true;

            gitleaks.enable = true;

            convco.enable = true;

            end-of-file-fixer = {
              enable = true;
              excludes = [
                "^CHANGELOG\\.md$"
                migrations
              ];
            };
            trim-trailing-whitespace = {
              enable = true;
              excludes = [
                "^CHANGELOG\\.md$"
                migrations
              ];
            };
            check-merge-conflicts = {
              enable = true;
              args = [ "--assume-in-merge" ];
            };
            check-added-large-files = {
              enable = true;
              args = [ "--enforce-all" ];
            };
            detect-private-keys.enable = true;

            typos = {
              enable = true;
              files = "\\.md$";
              excludes = [ "^CHANGELOG\\.md$" ];
            };
            markdownlint = {
              enable = true;
              excludes = [
                "^CHANGELOG\\.md$"
                "^apps/web/AGENTS\\.md$"
              ];
              settings.configuration = {
                default = true;
                MD013 = false;
                MD031.list_items = false;
                MD033 = false;
                MD060 = false;
              };
            };

            jactionlint = {
              enable = true;
              package = pkgs.jactionlint;
              entry = "${lib.getExe pkgs.jactionlint} --shellcheck=${lib.getExe pkgs.shellcheck} --pyflakes=${lib.getExe pkgs.python3Packages.pyflakes}";
              files = "^\\.github/|(^|/)action\\.ya?ml$";
              pass_filenames = false;
            };
          };

          packages.nestodo-cli = pkgs.callPackage ./apps/cli/package.nix { };

          devShells = {
            default = pkgs.mkShellNoCC {
              packages = [
                pkgs.nodejs_26
                pkgs.pnpm
              ]
              ++ config.pre-commit.settings.enabledPackages;

              shellHook = config.pre-commit.shellHook;
            };

            cli = pkgs.mkShell {
              packages = [
                pkgs.cargo
                pkgs.rustc
                pkgs.clippy
                pkgs.rustfmt
              ]
              ++ config.pre-commit.settings.enabledPackages;

              shellHook = config.pre-commit.shellHook;
            };
          }
          // lib.optionalAttrs (system != "aarch64-linux") {
            android = pkgs.mkShellNoCC {
              packages = [
                androidSdk
                jdk
              ]
              ++ config.pre-commit.settings.enabledPackages;

              ANDROID_HOME = "${androidSdk}/libexec/android-sdk";
              JAVA_HOME = jdk.home;

              shellHook = config.pre-commit.shellHook + ''
                app="$(git rev-parse --show-toplevel)/apps/android"
                props="$app/local.properties"
                if [ -d "$app" ] && ! grep -qxF "sdk.dir=$ANDROID_HOME" "$props" 2>/dev/null; then
                  { grep -v '^sdk\.dir=' "$props" 2>/dev/null || true; echo "sdk.dir=$ANDROID_HOME"; } > "$props.tmp"
                  mv "$props.tmp" "$props"
                fi
              '';
            };
          };
        };
    };
}
