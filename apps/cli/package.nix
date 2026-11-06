{ lib, rustPlatform }:

rustPlatform.buildRustPackage {
  pname = "nestodo-cli";
  version = (lib.importTOML ./Cargo.toml).package.version;

  src = lib.fileset.toSource {
    root = ../..;
    fileset = lib.fileset.unions [
      ../../Cargo.toml
      ../../Cargo.lock
      ./Cargo.toml
      ./src
    ];
  };
  cargoLock.lockFile = ../../Cargo.lock;

  doCheck = false;

  meta = {
    license = with lib.licenses; [
      mit
      asl20
    ];
    mainProgram = "nestodo";
  };
}
