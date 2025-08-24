### OPlus-Brightness-Adaptor-For-Transsion
A Quick Fix and simple for OPlus/Oppo/Realme Ports on Transsion Devices
by @rianixia on telegram, use credit when use.

The code is open source bc i decide to not gatekept this, if you know a better logic, better ways to do this please do tell me im all ears and feel free to give feedbacks im always listening despite my rough response. Thank you~

## DO NOT DELETE UR STOCK TRANSSION LIGHT HALS JUST ADD THIS ALONG WITH IT.

### How to Use
*Note : This was originally made for OS15 ROMs but if you want to try on OS14 or under i recommend you to add this prop*
```persist.sys.rianixia.brightness.isfloat=true```


# Step 1. Copy Everything to your vendor
File structure should looks like this
/vendor/bin/hw/vendor.xia.display.adaptor-V4@1.0-service
/vendor/etc/init/init.xia.display.adaptor.rc

# Step 3. Set the binary file context to
```u:object_r:mtk_hal_light_exec:s0```

# Final : Add this prop for debugging or completely skip if youre certain
```persist.sys.rianixia.display-debug=true```

# SEPolicy? 
None.
Address them yourself or use permissive each device has different sysfs labels

# IPS LCD Displays
For IPS LCD probably works as is. if not refer and try to AMOLED method 

# AMOLED Displays
but for AMOLED devices
make sure to check my_product and remove props that has these words : vrr, brightness, silky, underscreen
and add these props in vendor prop

```
#brightness prop
persist.sys.tran.brightness.gammalinear.convert=1
ro.transsion.tran_refresh_rate.support=1
ro.surface_flinger.set_idle_timer_ms=1200
sys.surfaceflinger.idle_reduce_framerate_enable=yes
ro.vendor.transsion.backlight_hal.optimization=1
ro.transsion.backlight.level=-1
ro.transsion.physical.backlight.optimization=1
ro.tran_90hz_refresh_rate.not_support=0
ro.tran_monitor_display_support=1
debug.camera.enhance_screen_brightness=0
ro.tran_refresh_rate_video_detector.support=1
ro.tran_low_battery_60hz_refresh_rate.support=0
ro.tran_default_auto_refresh.support=1
```
