//! Canonical transformation manifest and mass-balance rules.

use crate::error::ProtocolError;

use super::{
    FacilityId, TransformationId,
    asset::{LineageRole, require_nonzero_id},
    constants::MAX_MASS_TOLERANCE_BASIS_POINTS,
    hash::domain_hash,
    lineage::{LineageLeaf, merkle_root},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MassBalance {
    pub input_weight_grams: u64,
    pub output_weight_grams: u64,
    pub byproduct_weight_grams: u64,
    pub loss_weight_grams: u64,
    pub tolerance_basis_points: u16,
}

impl MassBalance {
    pub fn validate(self) -> Result<(), ProtocolError> {
        if self.tolerance_basis_points > MAX_MASS_TOLERANCE_BASIS_POINTS {
            return Err(ProtocolError::InvalidMassTolerance);
        }
        if self.input_weight_grams == 0 {
            return Err(ProtocolError::InvalidMassBalance);
        }

        let produced = u128::from(self.output_weight_grams)
            .checked_add(u128::from(self.byproduct_weight_grams))
            .and_then(|value| value.checked_add(u128::from(self.loss_weight_grams)))
            .ok_or(ProtocolError::MassOverflow)?;
        let input = u128::from(self.input_weight_grams);
        let difference = input.abs_diff(produced);
        let allowed = input
            .checked_mul(u128::from(self.tolerance_basis_points))
            .ok_or(ProtocolError::MassOverflow)?
            / 10_000;
        if difference > allowed {
            return Err(ProtocolError::MassBalanceOutsideTolerance);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransformationManifest {
    pub transformation_id: TransformationId,
    pub facility_id: FacilityId,
    pub transformation_type: u16,
    pub input_root: [u8; 32],
    pub output_root: [u8; 32],
    pub input_count: u32,
    pub output_count: u32,
    pub mass: MassBalance,
    pub manifest_nonce: u64,
    pub expires_at: i64,
}

impl TransformationManifest {
    pub const CANONICAL_LEN: usize = 32 + 32 + 2 + 32 + 32 + 4 + 4 + (8 * 4) + 2 + 8 + 8;

    pub fn validate(&self) -> Result<(), ProtocolError> {
        require_nonzero_id(&self.transformation_id)?;
        require_nonzero_id(&self.facility_id)?;
        if self.transformation_type == 0
            || self.input_root == [0; 32]
            || self.output_root == [0; 32]
            || self.input_count == 0
            || self.output_count == 0
            || self.expires_at < 0
        {
            return Err(ProtocolError::InvalidTransformationManifest);
        }
        self.mass.validate()
    }

    pub fn encode(&self) -> Result<[u8; Self::CANONICAL_LEN], ProtocolError> {
        self.validate()?;
        let mut bytes = [0u8; Self::CANONICAL_LEN];
        let mut offset = 0;
        bytes[offset..offset + 32].copy_from_slice(&self.transformation_id);
        offset += 32;
        bytes[offset..offset + 32].copy_from_slice(&self.facility_id);
        offset += 32;
        bytes[offset..offset + 2].copy_from_slice(&self.transformation_type.to_le_bytes());
        offset += 2;
        bytes[offset..offset + 32].copy_from_slice(&self.input_root);
        offset += 32;
        bytes[offset..offset + 32].copy_from_slice(&self.output_root);
        offset += 32;
        bytes[offset..offset + 4].copy_from_slice(&self.input_count.to_le_bytes());
        offset += 4;
        bytes[offset..offset + 4].copy_from_slice(&self.output_count.to_le_bytes());
        offset += 4;
        bytes[offset..offset + 8].copy_from_slice(&self.mass.input_weight_grams.to_le_bytes());
        offset += 8;
        bytes[offset..offset + 8].copy_from_slice(&self.mass.output_weight_grams.to_le_bytes());
        offset += 8;
        bytes[offset..offset + 8].copy_from_slice(&self.mass.byproduct_weight_grams.to_le_bytes());
        offset += 8;
        bytes[offset..offset + 8].copy_from_slice(&self.mass.loss_weight_grams.to_le_bytes());
        offset += 8;
        bytes[offset..offset + 2].copy_from_slice(&self.mass.tolerance_basis_points.to_le_bytes());
        offset += 2;
        bytes[offset..offset + 8].copy_from_slice(&self.manifest_nonce.to_le_bytes());
        offset += 8;
        bytes[offset..offset + 8].copy_from_slice(&self.expires_at.to_le_bytes());
        Ok(bytes)
    }

    pub fn manifest_hash(&self) -> Result<[u8; 32], ProtocolError> {
        Ok(domain_hash(
            super::constants::HASH_DOMAIN_TRANSFORMATION,
            &self.encode()?,
        ))
    }

    pub fn roots_from_leaves(
        inputs: &[LineageLeaf],
        outputs: &[LineageLeaf],
    ) -> Result<([u8; 32], [u8; 32]), ProtocolError> {
        if inputs.iter().any(|leaf| leaf.role != LineageRole::Input)
            || outputs
                .iter()
                .any(|leaf| !matches!(leaf.role, LineageRole::Output | LineageRole::Byproduct))
        {
            return Err(ProtocolError::InvalidLineageRole);
        }
        Ok((merkle_root(inputs)?, merkle_root(outputs)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(mass: MassBalance) -> TransformationManifest {
        TransformationManifest {
            transformation_id: [1; 32],
            facility_id: [2; 32],
            transformation_type: 1,
            input_root: [3; 32],
            output_root: [4; 32],
            input_count: 2,
            output_count: 3,
            mass,
            manifest_nonce: 9,
            expires_at: 100,
        }
    }

    #[test]
    fn balanced_manifest_is_fixed_width_and_hashable() {
        let value = manifest(MassBalance {
            input_weight_grams: 1_000,
            output_weight_grams: 800,
            byproduct_weight_grams: 150,
            loss_weight_grams: 50,
            tolerance_basis_points: 0,
        });
        assert_eq!(
            value.encode().unwrap().len(),
            TransformationManifest::CANONICAL_LEN
        );
        assert_ne!(value.manifest_hash().unwrap(), [0; 32]);
    }

    #[test]
    fn mass_outside_tolerance_is_rejected() {
        let error = manifest(MassBalance {
            input_weight_grams: 1_000,
            output_weight_grams: 700,
            byproduct_weight_grams: 100,
            loss_weight_grams: 50,
            tolerance_basis_points: 100,
        })
        .validate()
        .unwrap_err();
        assert_eq!(error, ProtocolError::MassBalanceOutsideTolerance);
    }

    #[test]
    fn manifest_hash_changes_when_mass_changes() {
        let first = manifest(MassBalance {
            input_weight_grams: 1_000,
            output_weight_grams: 800,
            byproduct_weight_grams: 150,
            loss_weight_grams: 50,
            tolerance_basis_points: 0,
        });
        let second = TransformationManifest {
            mass: MassBalance {
                output_weight_grams: 799,
                tolerance_basis_points: 100,
                ..first.mass
            },
            ..first
        };
        assert_ne!(
            first.manifest_hash().unwrap(),
            second.manifest_hash().unwrap()
        );
    }

    #[test]
    fn checked_in_fixture_matches_rust_canonicalization() {
        let inputs = [
            LineageLeaf {
                asset_id: [0x11; 32],
                role: LineageRole::Input,
                position: 0,
                quantity: 1,
                weight_grams: 600,
            },
            LineageLeaf {
                asset_id: [0x12; 32],
                role: LineageRole::Input,
                position: 1,
                quantity: 1,
                weight_grams: 400,
            },
        ];
        let outputs = [
            LineageLeaf {
                asset_id: [0x21; 32],
                role: LineageRole::Output,
                position: 0,
                quantity: 1,
                weight_grams: 800,
            },
            LineageLeaf {
                asset_id: [0x22; 32],
                role: LineageRole::Byproduct,
                position: 1,
                quantity: 1,
                weight_grams: 150,
            },
        ];
        let (input_root, output_root) =
            TransformationManifest::roots_from_leaves(&inputs, &outputs).expect("fixture roots");
        let manifest = TransformationManifest {
            transformation_id: [1; 32],
            facility_id: [2; 32],
            transformation_type: 1,
            input_root,
            output_root,
            input_count: 2,
            output_count: 2,
            mass: MassBalance {
                input_weight_grams: 1_000,
                output_weight_grams: 800,
                byproduct_weight_grams: 150,
                loss_weight_grams: 50,
                tolerance_basis_points: 0,
            },
            manifest_nonce: 9,
            expires_at: 100,
        };
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../schemas/fixtures/v2/transformation-manifest.json"
        ))
        .expect("valid transformation fixture");
        assert_eq!(fixture["canonicalLength"], 188);
        assert_eq!(fixture["manifest"]["inputRootHex"], hex::encode(input_root));
        assert_eq!(
            fixture["manifest"]["outputRootHex"],
            hex::encode(output_root)
        );
        assert_eq!(
            fixture["manifestHashHex"],
            hex::encode(manifest.manifest_hash().unwrap())
        );
    }
}
