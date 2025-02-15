{
  lib,
  config,
  ...
}:
with config.lib.stylix.colors.withHashtag;
with config.stylix.fonts;
let
  moxalertOpacity = lib.toHexString (
    ((builtins.ceil (config.stylix.opacity.popups * 100)) * 255) / 100
  );
in
{
  options.stylix.targets.moxalert.enable = config.lib.stylix.mkEnableTarget "moxalert" true;

  config = lib.mkIf (config.stylix.enable && config.stylix.targets.moxalert.enable) {
    services.moxalert.settings.styles = {
      default = {
        urgency_low = {
          background = base00 + moxalertOpacity;
          foreground = base05;
          border = base0B;
        };

        urgency_normal = {
          background = base01 + moxalertOpacity;
          foreground = base05;
          border = base0E;
        };

        urgency_critical = {
          background = base01 + moxalertOpacity;
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
        urgency_low.background = base02 + moxalertOpacity;
        urgency_normal.background = base02 + moxalertOpacity;
        urgency_critical.background = base02 + moxalertOpacity;
      };
    };
  };
}
