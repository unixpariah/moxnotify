{
  pkg-config,
  lua5_4,
  lib,
  rustPlatform,
  libxkbcommon,
  wayland,
  vulkan-loader,
}:

let
  cargoToml = builtins.fromTOML (builtins.readFile ../moxnotifyd/Cargo.toml);
in
rustPlatform.buildRustPackage rec {
  pname = "moxnotify";
  version = cargoToml.package.version;

  cargoLock.lockFile = ../Cargo.lock;

  src = lib.cleanSourceWith {
    src = ../.;
    filter =
      path: type:
      let
        relPath = lib.removePrefix (toString ../. + "/") (toString path);
      in
      lib.any (p: lib.hasPrefix p relPath) [
        "moxnotifyd"
        "moxnotifyctl"
        "contrib"
        "pl.mox.notify.service.in"
        "Cargo.toml"
        "Cargo.lock"
      ];
  };

  nativeBuildInputs = [
    pkg-config
  ];

  buildInputs = [
    lua5_4
    libxkbcommon
    wayland
    vulkan-loader
  ];

  buildPhase = ''
    cargo build --release --workspace
  '';

  installPhase = ''
    install -Dm755 target/release/moxnotifyd $out/bin/moxnotifyd
    install -Dm755 target/release/moxnotifyctl $out/bin/moxnotifyctl
  '';

  postFixup = ''
    mkdir -p $out/share/systemd/user
    substitute $src/contrib/systemd/moxnotify.service.in $out/share/systemd/user/moxnotify.service --replace-fail '@bindir@' "$out/bin"
    chmod 0644 $out/share/systemd/user/moxnotify.service

    mkdir -p $out/lib/systemd
    ln -s $out/share/systemd/user $out/lib/systemd/user

    mkdir -p $out/share/dbus-1/services
    substitute $src/pl.mox.notify.service.in $out/share/dbus-1/services/pl.mox.notify.service \
      --replace-fail '@bindir@' "$out/bin"
    chmod 0644 $out/share/dbus-1/services/pl.mox.notify.service

    patchelf --set-rpath "${lib.makeLibraryPath buildInputs}" $out/bin/moxnotifyd
  '';

  dontPatchELF = false;
  autoPatchelf = true;

  meta = with lib; {
    description = "Mox desktop environment notification system";
    homepage = "https://github.com/unixpariah/moxnotify";
    license = licenses.gpl3;
    maintainers = [ maintainers.unixpariah ];
    platforms = platforms.linux;
    mainProgram = "moxnotify";
  };
}
