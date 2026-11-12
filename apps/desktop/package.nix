{
  lib,
  stdenv,
  rustPlatform,
  imagemagick,
  python3Packages,
}:

let
  version = (lib.importTOML ./Cargo.toml).package.version;
  src = lib.fileset.toSource {
    root = ../..;
    fileset = lib.fileset.unions [
      ./Cargo.toml
      ./Cargo.lock
      ./Info.plist
      ./src
      ../web/public/icon.svg
      ../web/public/icon-512.png
      ../../THIRD_PARTY_NOTICES.txt
    ];
  };
in
rustPlatform.buildRustPackage {
  pname = "nestodo-desktop";
  inherit version src;
  sourceRoot = "${src.name}/apps/desktop";

  cargoLock.lockFile = ./Cargo.lock;

  nativeBuildInputs = [
    rustPlatform.bindgenHook
    imagemagick
    python3Packages.icnsutil
  ];

  doCheck = false;

  postInstall = ''
    app=$out/Applications/Nestodo.app
    mkdir -p $app/Contents/MacOS $app/Contents/Resources
    mv $out/bin/nestodo-desktop $app/Contents/MacOS/Nestodo
    rmdir $out/bin

    substitute Info.plist $app/Contents/Info.plist \
      --subst-var-by version ${version} \
      --subst-var-by minimumSystemVersion ${stdenv.hostPlatform.darwinMinVersion}

    mkdir iconset
    for icon in icp4:16 ic11:32 icp5:32 ic12:64 ic07:128 ic13:256 ic08:256 ic14:512 ic09:512 ic10:1024; do
      size=''${icon#*:}
      magick ../web/public/icon-512.png -resize "''${size}x''${size}" "iconset/''${icon%:*}.png"
    done
    icnsutil compose $app/Contents/Resources/Nestodo.icns iconset/*.png

    cp ../../THIRD_PARTY_NOTICES.txt $app/Contents/Resources/
  '';

  meta = {
    description = "Manage your todos in a native macOS window";
    homepage = "https://github.com/yagipass/nestodo";
    license = with lib.licenses; [
      mit
      asl20
      ofl
    ];
    platforms = lib.platforms.darwin;
  };
}
