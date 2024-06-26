use std::fs;
use std::ffi::OsStr;
use std::path::PathBuf;
use log::{debug, error};
use windows::core::{Result, GUID};
use windows::Win32::UI::Input::KeyboardAndMouse::{ActivateKeyboardLayout, GetKeyboardLayoutList, GetKeyboardLayoutNameA, KLF_SETFORPROCESS};
use windows::Win32::UI::TextServices::{self, HKL};
use windows::Win32::{System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER}, UI::TextServices::{ITfInputProcessorProfiles, CLSID_TF_InputProcessorProfiles, ITfCategoryMgr, CLSID_TF_CategoryMgr}};
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use winreg::RegKey;
use crate::extend::GUIDExt;
use crate::{global::*, extend::OsStrExt2};
use Layout::*;

//----------------------------------------------------------------------------
//
//  Registation for standard COM in-proc servers of any kind.
//  An IME is one of these servers.
//
//----------------------------------------------------------------------------


// FIXME these unwrappings...
pub unsafe fn register_server() -> Result<()> {
    // Register the IME's ASCII name under HKLM\SOFTWARE\Classes\CLSID\{IME_ID}
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let path = format!("SOFTWARE\\Classes\\CLSID\\{{{}}}", IME_ID.to_rfc4122());
    let (clsid, _) = hklm.create_subkey(path).unwrap();
    clsid.set_value("", &IME_NAME_ASCII).unwrap();
    // Register the dll's path under HKLM\SOFTWARE\Classes\CLSID\{IME_ID}\InprocServer32 
    let (inproc_server_32, _) = clsid.create_subkey("InprocServer32").unwrap();
    inproc_server_32.set_value("", &dll_path()?).unwrap();
    // Register the threading model under HKLM\SOFTWARE\Classes\CLSID\{IME_ID}\InprocServer32
    inproc_server_32.set_value("ThreadingModel", &"Apartment").unwrap();
    Ok(())
}

pub unsafe fn unregister_server() -> Result<()> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let path = format!("SOFTWARE\\Classes\\CLSID\\{{{}}}", IME_ID.to_rfc4122());
    hklm.delete_subkey_all(path).unwrap();
    Ok(())
}

//----------------------------------------------------------------------------
//
//  Registration for an IME.
//
//----------------------------------------------------------------------------


// features supported by the IME. there'are 18 of them in total. 
// register all of them expect the speech one and the handwriting one, or 
// your input method won't work in certain applications (for example, MS Word)
const SUPPORTED_CATEGORIES: [GUID; 16] = [
    TextServices::GUID_TFCAT_CATEGORY_OF_TIP,
    TextServices::GUID_TFCAT_TIP_KEYBOARD,
    // TextServices::GUID_TFCAT_TIP_SPEECH,
    // TextServices::GUID_TFCAT_TIP_HANDWRITING,
    TextServices:: GUID_TFCAT_TIPCAP_SECUREMODE,
    TextServices::GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
    TextServices::GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT,
    TextServices::GUID_TFCAT_TIPCAP_COMLESS,
    TextServices::GUID_TFCAT_TIPCAP_WOW16,
    TextServices::GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
    TextServices::GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
    TextServices::GUID_TFCAT_PROP_AUDIODATA,
    TextServices:: GUID_TFCAT_PROP_INKDATA,
    TextServices::GUID_TFCAT_PROPSTYLE_STATIC,
    GUID::from_u128(0x85F9794B_4D19_40D8_8864_4E747371A66D), // TextServices::GUID_TFCAT_PROPSTYLE_STATICCOMPSCT,
    GUID::from_u128(0x24AF3031_852D_40A2_BC09_8992898CE722), // TextServices::GUID_TFCAT_PROSTYLE_CUSTOM
    TextServices::GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
    TextServices::GUID_TFCAT_DISPLAYATTRIBUTEPROPERTY
];

pub unsafe fn register_ime() -> Result<()> {
    // some COM nonsense to create the registry objects.
    let input_processor_profiles: ITfInputProcessorProfiles = CoCreateInstance(
        &CLSID_TF_InputProcessorProfiles, 
        None, 
        CLSCTX_INPROC_SERVER)?;
    let category_mgr: ITfCategoryMgr = CoCreateInstance(
        &CLSID_TF_CategoryMgr, 
        None, 
        CLSCTX_INPROC_SERVER)?;
    let (lang_id, layout) = detect_layout();

    // three things to register:
    // 1. the IME itself
    // 2. language profile
    // 3. categories(the features the IME has)
    input_processor_profiles.Register(&IME_ID)?;
    debug!("Registered the input method.");
    let ime_name: Vec<u16> = OsStr::new(IME_NAME).null_terminated_wchars();
    let icon_file: Vec<u16> = dll_path()?.null_terminated_wchars();
    let icon_index = {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let path = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize";
        hkcu.open_subkey(path)
            .and_then(|subkey| subkey.get_value("SystemUsesLightTheme"))
            .map(|light_theme: u32| if light_theme == 1 { LITE_TRAY_ICON_INDEX } else { DARK_TRAY_ICON_INDEX })
            .unwrap_or(LITE_TRAY_ICON_INDEX)
    };
    input_processor_profiles.AddLanguageProfile(
        &IME_ID, lang_id, &LANG_PROFILE_ID, &ime_name, 
        &icon_file, icon_index)?;
    if let Some(layout) = layout {
        input_processor_profiles.SubstituteKeyboardLayout(&IME_ID, lang_id, &LANG_PROFILE_ID, layout)?;
    }
    debug!("Registered the language profile.");
    for rcatid  in SUPPORTED_CATEGORIES {
        category_mgr.RegisterCategory(&IME_ID, &rcatid, &IME_ID)?;
    }
    debug!("Registered the categories.");
    Ok(())
}

// similar process but re-doing everything
pub unsafe fn unregister_ime() -> Result<()> {
    let input_processor_profiles: ITfInputProcessorProfiles = CoCreateInstance(
        &CLSID_TF_InputProcessorProfiles, // using ::IID would cause unregister to fail
        None, 
        CLSCTX_INPROC_SERVER)?;
    let category_mgr: ITfCategoryMgr = CoCreateInstance(
        &CLSID_TF_CategoryMgr, 
        None, 
        CLSCTX_INPROC_SERVER)?;
    let (lang_id, _) = detect_layout();

    for rcatid in SUPPORTED_CATEGORIES {
        category_mgr.UnregisterCategory(&IME_ID, &rcatid, &IME_ID)?;
    }
    debug!("Unregistered the categories.");
    input_processor_profiles.RemoveLanguageProfile(&IME_ID, lang_id, &LANG_PROFILE_ID)?;
    debug!("Unregistered the language profile.");
    input_processor_profiles.Unregister(&IME_ID)?;
    debug!("Unregistered the input method.");
    Ok(())
}


//----------------------------------------------------------------------------
//
//  Detection of keyboard layouts
//
//----------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Layout {
    Qwerty, Dvorak,
    Qwertz, Azerty, 
    Custom
}

// QWERTY
const US: u16 = 0x0409;
// DVORAK
const US_DVORAK: u32 = 0x0001_0409;
// AZERTY
const FRENCH: u32 = 0x0000_040C;
const BELGIAN_FRENCH: u32 = 0x0000_080C;
const BELGIAN_FRENCH_COMMA: u32 = 0x0001_080C;
const BELGIAN_FRENCH_PERIOD: u32 = 0x0000_0813;
// QWERTZ
const GERMAN: u32 = 0x0000_0407;
const GERMAN_IBM: u32 = 0x0001_0407;
const SWISS_FRENCH: u32 = 0x0000_100C;


/// Detect the proper language ID and keyboard layout for the input method.
fn detect_layout() -> (u16, Option<HKL>) {
    match detect_layout_inner() {
        Some((lang_id, layout)) => (lang_id, Some(layout)),
        None => (US, None)
    }
}

/// Detect if there's any preferred keyboard layout.
fn detect_layout_inner() -> Option<(u16, HKL)> {
    let mut path = PathBuf::from(dll_path().ok()?);
    path.pop();
    path.push(".layout");
    

    // let path = PathBuf::from(env::var("APPDATA").ok()?);
    // let path = path.join(IME_NAME).join(".layout");

    let prefered_layout = fs::read_to_string(path); 
    let prefered_layout = prefered_layout.as_ref().map(|s|s.as_str());
    let prefered_layout = match prefered_layout {
        Err(e) => {
            error!("Keyboard layout is not specified. {e}");
            return None;
        }
        Ok("QWERTY") => Qwerty,
        Ok("DVORAK") => Dvorak,
        Ok("QWERTZ") => Qwertz,
        Ok("AZERTY") => Azerty,
        Ok("CUSTOM") => Custom,
        Ok(unrecognizable) => {
            error!("Unrecognizable layout: {unrecognizable}. ");
            return None;
        }
    };


    let mut hkls = [HKL::default(); 16];
    let len = unsafe { GetKeyboardLayoutList(Some(&mut hkls)) } as usize;
    let hkls = &hkls[..len];
    for hkl in hkls.iter().cloned() {
        let id = unsafe {
            let mut buf = [0; 9];
            ActivateKeyboardLayout(hkl, KLF_SETFORPROCESS).ok()?;
            GetKeyboardLayoutNameA(&mut buf).ok()?;
            u32::from_str_radix(std::str::from_utf8_unchecked(&buf[..8]), 16).ok()?
        };

        let layout = match id {
            US_DVORAK => Dvorak,
            GERMAN | GERMAN_IBM | SWISS_FRENCH => Qwertz,
            FRENCH | BELGIAN_FRENCH | BELGIAN_FRENCH_COMMA | BELGIAN_FRENCH_PERIOD => Azerty,
            _ => if id >> 28 == 0xA {
                Custom
            } else {
                Qwerty
            }
        };

        if layout == prefered_layout {
            debug!("Detected layouts: {id:08X?}");
            let lang_id = id as u16;
            return Some((lang_id, hkl));
        }
    }
    error!("Prefered layout is not found within the layout list of the OS. ");
    None
}