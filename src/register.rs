use std::ffi::{OsStr, OsString};
use log::debug;
use windows::core::{Result, GUID};
use windows::Win32::{System::{Com::{CoCreateInstance, CLSCTX_INPROC_SERVER}, LibraryLoader::GetModuleFileNameA}, UI::TextServices::{ITfInputProcessorProfiles, CLSID_TF_InputProcessorProfiles, ITfCategoryMgr, CLSID_TF_CategoryMgr, GUID_TFCAT_CATEGORY_OF_TIP, GUID_TFCAT_TIP_KEYBOARD, GUID_TFCAT_TIPCAP_SECUREMODE, GUID_TFCAT_TIPCAP_UIELEMENTENABLED, GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT, GUID_TFCAT_TIPCAP_COMLESS, GUID_TFCAT_TIPCAP_WOW16, GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT, GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT, GUID_TFCAT_PROP_AUDIODATA, GUID_TFCAT_PROP_INKDATA, GUID_TFCAT_PROPSTYLE_STATIC, GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER, GUID_TFCAT_DISPLAYATTRIBUTEPROPERTY}};
use winreg::{RegKey, enums::HKEY_CURRENT_USER};
use crate::{global::*, extend::OsStrExt2};


//----------------------------------------------------------------------------
//
//  Registation for standard COM in-proc servers of any kind.
//  An IME is one of these servers.
//
//----------------------------------------------------------------------------


// FIXME 无法注册到注册表中
// FIXME these unwrappings...
pub unsafe fn register_server() -> Result<()> {
    return Ok(());

    // Register the IME's ASCII name under HKEY_CLASSES_ROOT\CLSID\{IME_ID}
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let path = format!("SOFTWARE\\Classes\\CLSID\\{{{}}}", IME_ID);
    let (clsid, _) = hkcu.create_subkey(path).unwrap();
    clsid.set_value("", &IME_NAME_ASCII).unwrap();

    // Register the dll path under HKEY_CLASSES_ROOT\CLSID\{IME_ID}\InprocServer32 
    let (inproc_server_32, _) = clsid.create_subkey("InprocServer32").unwrap();
    let dll_path = {
        let mut buf: Vec<u8> = Vec::with_capacity(260);
        GetModuleFileNameA(DLL_MOUDLE.unwrap(), &mut buf);
        debug!("Buffer for GetModulFileNameA: {:?}", buf);
        OsString::from_encoded_bytes_unchecked(buf)
    };
    debug!("Dll path to be registered: {:?}", dll_path);
    inproc_server_32.set_value("", &dll_path).unwrap();

    // Register the threading model under HKEY_CLASSES_ROOT\{IME_ID}\InprocServer32
    inproc_server_32.set_value("ThreadingModel", &"Apartment").unwrap();
    Ok(())
}

pub unsafe fn unregister_server() -> Result<()> {
    return Ok(());
    
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let path = format!("SOFTWARE\\Classes\\CLSID\\{{{}}}", IME_ID);
    hkcu.delete_subkey_all(path).unwrap();
    Ok(())
}

//----------------------------------------------------------------------------
//
//  Registration for an IME.
//
//----------------------------------------------------------------------------


// features supported by the IME
const SUPPORTED_CATEGORIES: [GUID;16] = [
    GUID_TFCAT_CATEGORY_OF_TIP,
    GUID_TFCAT_TIP_KEYBOARD,
    GUID_TFCAT_TIPCAP_SECUREMODE,
    GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
    GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT,
    GUID_TFCAT_TIPCAP_COMLESS,
    GUID_TFCAT_TIPCAP_WOW16,
    GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
    GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
    GUID_TFCAT_PROP_AUDIODATA,
    GUID_TFCAT_PROP_INKDATA,
    GUID_TFCAT_PROPSTYLE_STATIC,
    GUID_TFCAT_PROPSTYLE_STATIC,
    GUID_TFCAT_PROPSTYLE_STATIC,
    GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
    GUID_TFCAT_DISPLAYATTRIBUTEPROPERTY
];

pub unsafe fn register_ime() -> Result<()> {
    // some COM nonsense to create the registry objects.
    let input_processor_profiles: ITfInputProcessorProfiles = CoCreateInstance(
        &CLSID_TF_InputProcessorProfiles as *const GUID, 
        None, 
        CLSCTX_INPROC_SERVER)?;
    let category_mgr: ITfCategoryMgr = CoCreateInstance(
        &CLSID_TF_CategoryMgr as *const GUID, 
        None, 
        CLSCTX_INPROC_SERVER)?;

    // three things to register:
    // 1. the IME itself
    // 2. language profile
    // 3. categories(the features the IME has)

    let ime_id = &GUID::from(IME_ID);
    let lang_profile_id = &GUID::from(LANG_PROFILE_ID) as *const GUID;

    input_processor_profiles.Register(ime_id)?;

    // todo the icon cannot be registered
    let ime_name: Vec<u16> = OsStr::new(IME_NAME).null_terminated_wchars();
    let icon_file: Vec<u16> = OsStr::new(ICON_FILE).null_terminated_wchars();
    input_processor_profiles.AddLanguageProfile(ime_id, LANG_ID, lang_profile_id, &ime_name, &icon_file, 0)?;

    for rcatid  in SUPPORTED_CATEGORIES {
        let rcatid = &rcatid as *const GUID;
        category_mgr.RegisterCategory(ime_id, rcatid, ime_id)?;
    }
    Ok(())
}

// similar process but re-doing everything
pub unsafe fn unregister_ime() -> Result<()> {
    // todo: it seems able to unregister the dll but alaways exits with 0x80004005
    let input_processor_profiles: ITfInputProcessorProfiles = CoCreateInstance(
        &CLSID_TF_InputProcessorProfiles as *const GUID, // using ::IID would cause unregister to fail
        None, 
        CLSCTX_INPROC_SERVER)?;
    let category_mgr: ITfCategoryMgr = CoCreateInstance(
        &CLSID_TF_CategoryMgr as *const GUID, 
        None, 
        CLSCTX_INPROC_SERVER)?;


    let ime_id = &GUID::from(IME_ID) as *const GUID;
    let lang_profile_id = &GUID::from(LANG_PROFILE_ID) as *const GUID;

    input_processor_profiles.Unregister(ime_id)?;
    input_processor_profiles.RemoveLanguageProfile(ime_id, LANG_ID, lang_profile_id)?;
    for rcatid in SUPPORTED_CATEGORIES {
        let rcatid = &rcatid as *const GUID;
        category_mgr.UnregisterCategory(ime_id, rcatid, ime_id)?;
    }
    Ok(())
}


