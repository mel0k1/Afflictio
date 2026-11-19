use crate::driver::BinderDevice;
use crate::parcel::ParcelWriter;
use crate::BinderError;

pub const SERVICE_MANAGER_DESCRIPTOR: &str = "android.os.IServiceManager";
pub const SERVICE_MANAGER_HANDLE: u32 = 0;
pub const SM_GET_SERVICE: u32 = 1;
pub const SM_CHECK_SERVICE: u32 = 2;

pub fn get_service(device: &BinderDevice, name: &str) -> Result<Option<u32>, BinderError> {
    let mut parcel = ParcelWriter::new();
    parcel.write_interface_token(SERVICE_MANAGER_DESCRIPTOR);
    parcel.write_string16(name);
    let reply = device
        .call(SERVICE_MANAGER_HANDLE, SM_GET_SERVICE, &parcel, false)?
        .ok_or(BinderError::FailedReply)?;
    Ok(reply.object.map(|object| object.handle))
}

pub fn check_service(device: &BinderDevice, name: &str) -> Result<Option<u32>, BinderError> {
    let mut parcel = ParcelWriter::new();
    parcel.write_interface_token(SERVICE_MANAGER_DESCRIPTOR);
    parcel.write_string16(name);
    let reply = device
        .call(SERVICE_MANAGER_HANDLE, SM_CHECK_SERVICE, &parcel, false)?
        .ok_or(BinderError::FailedReply)?;
    Ok(reply.object.map(|object| object.handle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parcel::{ParcelReader, BINDER_TYPE_HANDLE};

    #[test]
    fn get_service_request_layout() {
        let mut parcel = ParcelWriter::new();
        parcel.write_interface_token(SERVICE_MANAGER_DESCRIPTOR);
        parcel.write_string16("surfaceflinger");
        let raw = parcel.data();
        let mut reader = ParcelReader::new(raw, &[]);
        let token = reader.read_string16().unwrap();
        assert_eq!(token, SERVICE_MANAGER_DESCRIPTOR);
        let name = reader.read_string16().unwrap();
        assert_eq!(name, "surfaceflinger");
        assert_eq!(reader.position(), raw.len());
    }

    #[test]
    fn reply_handle_extraction() {
        let mut w = ParcelWriter::new();
        w.write_object(crate::parcel::handle_object(0x7));
        let (data, offsets) = w.into_parts();
        let reader = ParcelReader::new(&data, &offsets);
        let object = reader.first_object().unwrap();
        assert_eq!(object.kind, BINDER_TYPE_HANDLE);
        assert_eq!(object.handle, 0x7);
    }
}
