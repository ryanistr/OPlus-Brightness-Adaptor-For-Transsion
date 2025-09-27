### OPlus-Brightness-Adaptor-For-Transsion

A quick fix and simple solution for OPlus/Oppo/Realme ports on Transsion devices.
*By @rianixia on Telegram. Please give credit when using.*

The code is open source because I decided not to gatekeep it. If you know a better logic or method, please share! I'm always listening to feedback—even if my responses are rough. Thank you\~

### Download
**[GitHub Releases page](https://github.com/ryanistr/OPlus-Brightness-Adaptor-For-Transsion/releases/tag/4.0)**.


## ⚠️ Important

**DO NOT DELETE your stock Transsion light HALs.**
Just add this alongside them.

---

## How to Use

> **Note:** This Guide is for ColorOS, RealmeUI, OxygenOS14 Ports


### Step 1: Copy Files on OS14 Folder in the zip to Vendor, including displayfeature

File structure should look like this:

```
/vendor/bin/hw/vendor.xia.display.adaptor-V4@1.0-service
/vendor/etc/init/init.xia.display.adaptor.rc
/vendor/odm/(all files on odm)
```

### Step 2: Set Binary File Context to mtk_light on fs_context

```bash
u:object_r:mtk_hal_light_exec:s0
```

### Step 3: Add a Prop to tell the binary you're on OS14

```bash
persist.sys.rianixia.is-displaypanel.support=true
# OS14 Brightness min max value (NOT YOUR DEVICE MIN MAX) keep default if unsure
persist.sys.rianixia-display.max=5118
persist.sys.rianixia-display.min=22
```

### Step 4: Enable debug if you are having issues

```bash
persist.sys.rianixia.display-debug=true
```

Check logs with

```bash
logcat | grep Xia
```
## SEPolicy

None.
Address manually or use permissive mode. Each device may have different sysfs labels.

---

## Display Types

### IPS LCD Displays

Should work as-is. If not, refer to AMOLED method.

### AMOLED Displays

Make sure to check `my_product` and remove props containing: `vrr`, `brightness`, `silky`, `underscreen`. Then add the following props in vendor properties:

```bash
# Brightness Props for AMOLED
persist.sys.tran.brightness.gammalinear.convert=1
ro.vendor.transsion.backlight_hal.optimization=1
ro.transsion.backlight.level=-1
ro.transsion.physical.backlight.optimization=1
```
Note:DO NOT DELETE PROPS THAT HAS "MIN" or "MAX" IN THE PROP
