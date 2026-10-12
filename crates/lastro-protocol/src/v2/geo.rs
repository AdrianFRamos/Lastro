//! Facility boundary commitment (SPEC v2.1, S5).
//!
//! A facility's boundary is its CAR code plus a polygon. On-chain the `FacilityRecord` stores
//! only `geo_commitment`; the boundary travels in the EvidencePackage so any verifier can
//! recompute it. JSON canonicalization differs across languages, so the commitment is over a
//! fixed binary encoding instead:
//!
//! ```text
//! car_code (1..=100 printable ASCII bytes) || 0x00
//! ring_count u32 LE (1..=64)
//! per ring: vertex_count u32 LE (4..), then vertex_count x (lon_e7 i32 LE, lat_e7 i32 LE)
//! ```
//!
//! Coordinates are WGS84 degrees x 10^7. Ring 0 is the outer boundary, the others are holes;
//! every ring is closed (first vertex equals last). At most 10 000 vertices in total.

use super::hash::domain_hash;
use crate::error::ProtocolError;

pub const HASH_DOMAIN_GEO: &[u8] = b"LASTRO_V2_GEO\0";
pub const MAX_CAR_CODE_LEN: usize = 100;
pub const MAX_BOUNDARY_RINGS: usize = 64;
pub const MAX_BOUNDARY_VERTICES: usize = 10_000;
const MAX_LON_E7: i32 = 1_800_000_000;
const MAX_LAT_E7: i32 = 900_000_000;

/// One vertex: (longitude, latitude) in degrees x 10^7.
pub type VertexE7 = (i32, i32);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FacilityBoundary {
    pub car_code: String,
    /// Ring 0 is the outer boundary; further rings are holes. Each ring is closed.
    pub rings: Vec<Vec<VertexE7>>,
}

impl FacilityBoundary {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        let code = self.car_code.as_bytes();
        let vertices: usize = self.rings.iter().map(Vec::len).sum();
        let valid = !code.is_empty()
            && code.len() <= MAX_CAR_CODE_LEN
            && code.iter().all(|byte| (0x21..=0x7e).contains(byte))
            && !self.rings.is_empty()
            && self.rings.len() <= MAX_BOUNDARY_RINGS
            && vertices <= MAX_BOUNDARY_VERTICES
            && self.rings.iter().all(|ring| {
                ring.len() >= 4
                    && ring.first() == ring.last()
                    && ring.iter().all(|(lon, lat)| {
                        (-MAX_LON_E7..=MAX_LON_E7).contains(lon)
                            && (-MAX_LAT_E7..=MAX_LAT_E7).contains(lat)
                    })
            });
        if valid {
            Ok(())
        } else {
            Err(ProtocolError::InvalidBoundary)
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        self.validate()?;
        let vertices: usize = self.rings.iter().map(Vec::len).sum();
        let mut out =
            Vec::with_capacity(self.car_code.len() + 1 + 4 + self.rings.len() * 4 + vertices * 8);
        out.extend_from_slice(self.car_code.as_bytes());
        out.push(0);
        out.extend_from_slice(&(self.rings.len() as u32).to_le_bytes());
        for ring in &self.rings {
            out.extend_from_slice(&(ring.len() as u32).to_le_bytes());
            for (lon, lat) in ring {
                out.extend_from_slice(&lon.to_le_bytes());
                out.extend_from_slice(&lat.to_le_bytes());
            }
        }
        Ok(out)
    }

    /// The value the facility's on-chain `geo_commitment` must hold.
    pub fn commitment(&self) -> Result<[u8; 32], ProtocolError> {
        Ok(domain_hash(HASH_DOMAIN_GEO, &self.encode()?))
    }

    /// Even-odd point-in-polygon over all rings (holes excluded). Points on an edge count as
    /// inside. For future GNSS evidence; a commitment check never depends on it.
    pub fn contains(&self, point: VertexE7) -> bool {
        let mut inside = false;
        for ring in &self.rings {
            for edge in ring.windows(2) {
                let (a, b) = (edge[0], edge[1]);
                if on_segment(point, a, b) {
                    return true;
                }
                let (px, py) = (i64::from(point.0), i64::from(point.1));
                let (ax, ay, bx, by) = (
                    i64::from(a.0),
                    i64::from(a.1),
                    i64::from(b.0),
                    i64::from(b.1),
                );
                if (ay > py) != (by > py) {
                    // x of the edge at y = py, compared without division.
                    let lhs = (px - ax) * (by - ay);
                    let rhs = (bx - ax) * (py - ay);
                    if (by > ay && lhs < rhs) || (by < ay && lhs > rhs) {
                        inside = !inside;
                    }
                }
            }
        }
        inside
    }
}

fn on_segment(point: VertexE7, a: VertexE7, b: VertexE7) -> bool {
    let cross = (i64::from(b.0) - i64::from(a.0)) * (i64::from(point.1) - i64::from(a.1))
        - (i64::from(b.1) - i64::from(a.1)) * (i64::from(point.0) - i64::from(a.0));
    cross == 0
        && point.0 >= a.0.min(b.0)
        && point.0 <= a.0.max(b.0)
        && point.1 >= a.1.min(b.1)
        && point.1 <= a.1.max(b.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn farm() -> FacilityBoundary {
        // A 0.01-degree square near Cuiabá with a square hole.
        FacilityBoundary {
            car_code: "MT-5103403-0A1B2C3D4E5F".into(),
            rings: vec![
                vec![
                    (-561_000_000, -155_000_000),
                    (-560_900_000, -155_000_000),
                    (-560_900_000, -154_900_000),
                    (-561_000_000, -154_900_000),
                    (-561_000_000, -155_000_000),
                ],
                vec![
                    (-560_960_000, -154_960_000),
                    (-560_940_000, -154_960_000),
                    (-560_940_000, -154_940_000),
                    (-560_960_000, -154_940_000),
                    (-560_960_000, -154_960_000),
                ],
            ],
        }
    }

    #[test]
    fn the_commitment_binds_every_byte_of_the_boundary() {
        // PURPOSE: a verifier holding the boundary recomputes exactly the on-chain commitment.
        // FAILURE MEANS: a different polygon or CAR code could pass as the registered one.
        let boundary = farm();
        let commitment = boundary.commitment().unwrap();
        let mut moved = boundary.clone();
        moved.rings[0][1].0 += 1;
        moved.rings[0][4] = moved.rings[0][0];
        assert_ne!(moved.commitment().unwrap(), commitment);
        let mut other_car = boundary.clone();
        other_car.car_code.push('X');
        assert_ne!(other_car.commitment().unwrap(), commitment);
        assert_eq!(
            boundary.encode().unwrap().len(),
            23 + 1 + 4 + 2 * (4 + 5 * 8)
        );
    }

    #[test]
    fn invalid_boundaries_are_rejected() {
        let mut open_ring = farm();
        open_ring.rings[0].pop();
        assert!(open_ring.validate().is_err());
        let mut out_of_range = farm();
        out_of_range.rings[0][0].1 = 900_000_001;
        assert!(out_of_range.validate().is_err());
        let mut blank_code = farm();
        blank_code.car_code = "MT 51".into();
        assert!(blank_code.validate().is_err());
        assert!(
            FacilityBoundary {
                car_code: "X".into(),
                rings: vec![]
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn point_in_polygon_respects_holes_and_edges() {
        let boundary = farm();
        assert!(boundary.contains((-560_990_000, -154_990_000)));
        assert!(!boundary.contains((-560_950_000, -154_950_000))); // inside the hole
        assert!(!boundary.contains((-560_800_000, -154_950_000))); // east of the farm
        assert!(boundary.contains((-561_000_000, -154_950_000))); // on the west edge
    }
}
