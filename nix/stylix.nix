{ lib, config, ... }:
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

      styles = [
        {
          selector = "*";
          style = {
            font = {
              family = sansSerif.name;
              size = sizes.popups;
              color = base05;
            };
            border.color = {
              urgency_low = base0B;
              urgency_normal = base0E;
              urgency_critical = base08;
            };
          };
        }
        {
          selector = "next_counter";
          style.background = {
            urgency_low = base00 + moxnotifyOpacity;
            urgency_normal = base01 + moxnotifyOpacity;
            urgency_critical = base01 + moxnotifyOpacity;
          };
        }
        {
          selector = "prev_counter";
          style.background = {
            urgency_low = base00 + moxnotifyOpacity;
            urgency_normal = base01 + moxnotifyOpacity;
            urgency_critical = base01 + moxnotifyOpacity;
          };
        }
        {
          selector = "notification";
          style.background = {
            urgency_low = base00 + moxnotifyOpacity;
            urgency_normal = base01 + moxnotifyOpacity;
            urgency_critical = base01 + moxnotifyOpacity;
          };
        }
        {
          selector = "notification";
          state = "hover";
          style.background = {
            urgency_low = base02 + moxnotifyOpacity;
            urgency_normal = base02 + moxnotifyOpacity;
            urgency_critical = base02 + moxnotifyOpacity;
          };
        }
        {
          selector = "action";
          state = "hover";
          style.background = {
            urgency_low = base0F;
            urgency_normal = base0F;
            urgency_critical = base08;
          };
        }
        {
          selector = "progress";
          style.background = {
            urgency_low = base0F;
            urgency_normal = base0F;
            urgency_critical = base08;
          };
        }
        {
          selector = "dismiss";
          style.font.color = "#00000000";
        }
        {
          selector = "dismiss";
          state = "container_hover";
          style.font.color = "#000000";
        }
      ];
    };
  };
}
