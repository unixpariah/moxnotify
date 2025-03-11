{
  lib,
  config,
  ...
}:
with config.lib.stylix.colors.withHashtag;
with config.stylix.fonts;
let
  moxnotifyOpacity = lib.toHexString (
    ((builtins.floor (config.stylix.opacity.popups * 100 + 0.5)) * 255) / 100
  );
in
{
  options.stylix.targets.moxnotify.enable = config.lib.stylix.mkEnableTarget "moxnotify" true;

  config = lib.mkIf (config.stylix.enable && config.stylix.targets.moxnotify.enable) {
    services.moxnotify.settings = {
      styles = {
        default = {
          progress.complete_color = base0F;

          button.dismiss = {
            default = {
              background_color = base08;
              border_color = base08;
            };
            hover = {
              background_color = base07;
              border_color = base07;
            };
          };

          urgency_low = {
            background = base00 + moxnotifyOpacity;
            foreground = base05;
            border = base0B;
            icon_border = base0B;
          };

          urgency_normal = {
            background = base01 + moxnotifyOpacity;
            foreground = base05;
            border = base0E;
            icon_border = base0E;
          };

          urgency_critical = {
            background = base01 + moxnotifyOpacity;
            foreground = base05;
            border = base08;
            icon_border = base08;
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
  };
}
