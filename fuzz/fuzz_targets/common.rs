use volmap::format::{IO_PAGE_SIZE, PAGE_WATERMARK_SIZE, PageType};
use volmap::model::{PageId, VolId, Vpid};

pub fn vpid() -> Vpid {
    Vpid::new(VolId::new(0).unwrap(), PageId::new(0).unwrap())
}

pub fn page_vpid(page: &[u8; IO_PAGE_SIZE]) -> Option<Vpid> {
    let page_id = i32::from_le_bytes(page[8..12].try_into().ok()?);
    let vol_id = i16::from_le_bytes(page[12..14].try_into().ok()?);
    Some(Vpid::new(
        VolId::new(vol_id).ok()?,
        PageId::new(page_id).ok()?,
    ))
}

#[allow(dead_code)]
pub fn normalized_page(data: &[u8], kinds: &[PageType]) -> [u8; IO_PAGE_SIZE] {
    let mut page = [0_u8; IO_PAGE_SIZE];
    if data.len() == IO_PAGE_SIZE && kinds.iter().any(|kind| kind.ordinal() == data[14]) {
        page.copy_from_slice(data);
        return page;
    }
    let selector = data.first().copied().unwrap_or_default();
    let kind = kinds[usize::from(selector) % kinds.len()];
    let body = data.get(1..).unwrap_or_default();
    let length = body.len().min(IO_PAGE_SIZE);
    page[..length].copy_from_slice(&body[..length]);
    page[0..8].fill(0);
    page[8..12].copy_from_slice(&0_i32.to_le_bytes());
    page[12..14].copy_from_slice(&0_i16.to_le_bytes());
    page[14] = kind.ordinal();
    page[15] = 0;
    page[IO_PAGE_SIZE - PAGE_WATERMARK_SIZE..].fill(0);
    page
}
