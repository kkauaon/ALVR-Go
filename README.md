<p align="center"> <img width="500" src="resources/alvr_combined_logo_hq.png"/> </p>

# ALVR - Air Light VR

[![badge-discord][]][link-discord] [![badge-matrix][]][link-matrix] [![badge-opencollective][]][link-opencollective]

Stream VR games from your PC to your headset via Wi-Fi.  
ALVR uses technologies like [Asynchronous Timewarp](https://developer.oculus.com/documentation/native/android/mobile-timewarp-overview) and [Fixed Foveated Rendering](https://developer.oculus.com/documentation/native/android/mobile-ffr) for a smoother experience.  
All games that work with an Oculus Rift (s) should work with ALVR.  
This is a fork of [ALVR](https://github.com/polygraphene/ALVR).

|    Headset     |                        Support                         |
| :------------: | :----------------------------------------------------: |
|  Oculus Quest  |                   :heavy_check_mark:                   |
| Oculus Quest 2 |                   :heavy_check_mark:                   |
|   Oculus Go    |                   :heavy_check_mark:                   |
|     GearVR     | :x: (use [this](https://github.com/polygraphene/ALVR)) |

## Requirements

-   Oculus Quest, Oculus Quest 2 or Oculus Go on the latest firmware  

-   SteamVR  

-   High-end gaming PC
    -   Windows 10 May 2020 update is recommended. A browser is no longer needed: the dashboard is a native window.  
    -   Minimum supported OS version is Windows 8.  
    -   NVIDIA GPU that supports NVENC ([Supported GPUs](https://github.com/polygraphene/ALVR/wiki/Supported-GPU)) (or with an AMD GPU that supports AMF VCE) with the latest driver.  
    -   Laptops with an onboard (Intel HD, AMD iGPU) and an additional dedicated GPU (NVidia GTX/RTX, AMD HD/R5/R7): you should assign the dedicated GPU or "high performance graphics adapter" to the applications ALVR, SteamVR for best performance and compatibility. (NVidia: Nvidia control panel->3d settings->application settings; AMD: similiar way) 

-   802.11ac 5Ghz wireless or ethernet wired connection  
    -   It is recommended to use 802.11ac 5Ghz for the headset and ethernet for PC  
    -   You need to connect both the PC and the headset to same router (or use a routed connection as described [here](https://github.com/alvr-org/ALVR/wiki/ALVR-client-and-server-on-separate-networks))

## Install

Follow the installation guide [here](https://github.com/alvr-org/ALVR/wiki/Installation).

## Oculus Go controller

The Go controller only has a trigger, a clickable trackpad and a back button (its home button belongs to the headset and is never sent to the PC), so the client maps it to an Oculus Touch controller and emulates the missing buttons with the trackpad edges:

| Oculus Go                                    | Seen by SteamVR as                                                 |
| :------------------------------------------- | :----------------------------------------------------------------- |
| Trigger                                      | Trigger                                                            |
| Finger on the trackpad                       | Thumbstick (and A touch)                                           |
| Trackpad click, middle                       | **A** (**X** on the left hand)                                     |
| Trackpad click, bottom edge (hold)           | **Grip**: hold to grab, release to let go                          |
| Trackpad click, top edge                     | **Menu**                                                           |
| Back button, short press                     | **B** (**Y** on the left hand), sent when the button is released   |
| Back button, hold for 0.7 s                  | **System**: opens the SteamVR dashboard                            |

-   The part of the trackpad that is clicked is chosen when the click starts and does not change if the finger slides. While the grip or menu edge is pressed the thumbstick is held at zero.  
-   The edges are the top and bottom 30% of the trackpad and the hold time is 0.7 s. They are the constants `GO_TRACKPAD_TOP_EDGE`, `GO_TRACKPAD_BOTTOM_EDGE` and `GO_BACK_LONG_PRESS_DURATION` at the top of `alvr/client/android/app/src/main/cpp/ovr_context.cpp`.  
-   The mapping was written from the code and has not been tried on a real controller yet.  

## Troubleshooting

-   Please check the [Troubleshooting](https://github.com/alvr-org/ALVR/wiki/Troubleshooting) page. The original repository [wiki](https://github.com/polygraphene/ALVR/wiki/Troubleshooting) can also help.  
-   Configuration recommendations and information may be found [here](https://github.com/alvr-org/ALVR/wiki/Configuration-Information-and-Recommendations)

## Uninstall

Open `ALVR Launcher.exe`, go to `Installation` tab then press `Remove firewall rules`. Close ALVR window and delete the ALVR folder.

If you have a version prior to v12.0 you need to launch `remove_firewall_rules.bat` and `driver_uninstall.bat` in the installation folder.

## Build from source

You can follow the guide [here](https://github.com/alvr-org/ALVR/wiki/Building-From-Source).

## License

ALVR is licensed under the [MIT License](LICENSE).

## Privacy policy

ALVR apps do not directly collect any kind of data.

## Donate

If you want to support this project you can make a donation to our [Open Source Collective account](https://opencollective.com/alvr).

You can also donate to the original author of ALVR using Paypal (polygraphene@gmail.com) or with bitcoin (1FCbmFVSjsmpnAj6oLx2EhnzQzzhyxTLEv).

[badge-discord]: https://img.shields.io/discord/720612397580025886?style=for-the-badge&logo=discord&color=5865F2 "Join us on Discord"
[link-discord]: https://discord.gg/ALVR
[badge-matrix]: https://img.shields.io/static/v1?label=chat&message=%23alvr&style=for-the-badge&logo=matrix&color=blueviolet "Join us on Matrix"
[link-matrix]: https://matrix.to/#/#alvr:ckie.dev?via=ckie.dev
[badge-opencollective]: https://img.shields.io/opencollective/all/alvr?style=for-the-badge&logo=opencollective&color=79a3e6 "Donate"
[link-opencollective]: https://opencollective.com/alvr
