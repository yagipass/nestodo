{ lib, rustPlatform }:

let
  version = (lib.importTOML ./Cargo.toml).package.version;
in
rustPlatform.buildRustPackage {
  pname = "nestodo-cli";
  inherit version;

  src = lib.fileset.toSource {
    root = ./.;
    fileset = lib.fileset.unions [
      ./Cargo.toml
      ./Cargo.lock
      ./src
    ];
  };
  cargoLock.lockFile = ./Cargo.lock;

  doCheck = false;

  doInstallCheck = true;
  installCheckPhase = ''
    runHook preInstallCheck
    $out/bin/nestodo --version | grep -Fx 'nestodo ${version}'
    runHook postInstallCheck
  '';

  meta = {
    description = "Manage your todos from the terminal";
    homepage = "https://github.com/yagipass/nestodo";
    license = with lib.licenses; [
      mit
      asl20
    ];
    mainProgram = "nestodo";
  };
}
