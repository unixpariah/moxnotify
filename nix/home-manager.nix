{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.moxnotify;

  toLua =
    value:
    let
      recurse = v: toLua v;
      generators = {
        bool = b: if b then "true" else "false";
        int = toString;
        float = toString;
        string = s: ''"${lib.escape [ ''"'' ] s}"'';
        path = p: ''"${p}"'';
        null = "nil";
        list = vs: "{\n  ${lib.concatMapStringsSep ",\n  " recurse vs}\n}";
        attrs = vs: ''
          {
            ${lib.concatStringsSep ",\n" (
              lib.mapAttrsToList (k: v: "[${generators.string k}] = ${recurse v}") vs
            )}
          }'';
      };
    in
    if builtins.isAttrs value then
      generators.attrs value
    else if builtins.isList value then
      generators.list value
    else
      generators.${builtins.typeOf value} value;

in
{
  options.services.moxnotify = {
    enable = lib.mkEnableOption "moxnotify";
    package = lib.mkPackageOption pkgs "moxnotify" { };

    settings = lib.mkOption {
      type =
        with lib.types;
        let
          valueType = nullOr (oneOf [
            bool
            int
            float
            str
            path
            (attrsOf valueType)
            (listOf valueType)
          ]);
        in
        valueType;
      default = { };
      description = ''
        moxnotify configuration in Nix format that will be converted to Lua.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    xdg.configFile."moxnotify/config.lua" = lib.mkIf (cfg.settings != { }) {
      text = ''
        -- Generated by Home Manager
        return ${toLua cfg.settings}
      '';
    };

    home.packages = [ cfg.package ];

    systemd.user.services.moxnotify = {
      Install = {
        WantedBy = [ config.wayland.systemd.target ];
      };

      Unit = {
        Description = "moxnotify";
        PartOf = [ config.wayland.systemd.target ];
        After = [ config.wayland.systemd.target ];
        ConditionEnvironment = "WAYLAND_DISPLAY";
        X-Restart-Triggers = lib.mkIf (cfg.settings != { }) [
          config.xdg.configFile."moxnotify/config.lua".source
        ];
      };

      Service = {
        Type = "dbus";
        BusName = "org.freedesktop.Notifications";
        ExecStart = "${lib.getExe cfg.package}";
        Restart = "always";
        RestartSec = "10";
      };
    };
  };
}
