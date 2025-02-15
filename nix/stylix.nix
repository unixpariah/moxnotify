{
  lib,
  config,
  ...
}:
with config.lib.stylix.colors.withHashtag;
with config.stylix.fonts;
let
  moxnotifyOpacity = lib.toHexString (
    ((builtins.ceil (config.stylix.opacity.popups * 100)) * 255) / 100
  );
in
{
  options.stylix.targets.moxnotify.enable = config.lib.stylix.mkEnableTarget "moxnotify" true;

  config = lib.mkIf (config.stylix.enable && config.stylix.targets.moxnotify.enable) {
    services.moxnotify.settings.styles = {
      default = {
        urgency_low = {
          background = base00 + moxnotifyOpacity;
          foreground = base05;
          border = base0B;
        };

        urgency_normal = {
          background = base01 + moxnotifyOpacity;
          foreground = base05;
          border = base0E;
        };

        urgency_critical = {
          background = base01 + moxnotifyOpacity;
          foreground = base05;
          border = base08;
        };

        font = {
          family = sansSerif.name;
          size = sizes.popups;
        };
        border.size = 2;
      };
      hover = {
        urgency_low.background = base02 + moxnotifyOpacity;
        urgency_normal.background = base02 + moxnotifyOpacity;
        urgency_critical.background = base02 + moxnotifyOpacity;
      };
    };
  };
}
