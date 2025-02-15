{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.moxalert;

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
  options.services.moxalert = {
    enable = lib.mkEnableOption "moxalert";
    package = lib.mkPackageOption pkgs "moxalert" { };

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
        moxalert configuration in Nix format that will be converted to Lua.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    xdg.configFile."moxalert/config.lua" = lib.mkIf (cfg.settings != { }) {
      text = ''
        -- Generated by Home Manager
        return ${toLua cfg.settings}
      '';
    };

    home.packages = [ cfg.package ];

    systemd.user.services.moxalert = {
      Install = {
        WantedBy = [ config.wayland.systemd.target ];
      };

      Unit = {
        Description = "moxalert";
        PartOf = [ config.wayland.systemd.target ];
        After = [ config.wayland.systemd.target ];
        ConditionEnvironment = "WAYLAND_DISPLAY";
        X-Restart-Triggers = [ config.xdg.configFile."moxalert/config.lua".source ];
      };

      Service = {
        ExecStart = "${lib.getExe cfg.package}";
        Restart = "always";
        RestartSec = "10";
      };
    };
  };
}
