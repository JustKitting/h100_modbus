//! Source-backed decoder for H100 input register 000A (Current fault).
//!
//! Raw values and suffix ordering come from the input-register 000A table on
//! printed page 84 of H100 manual V1.8. Fault meanings and countermeasures
//! come from that manual's fault-information table. Values absent from those
//! source tables remain explicitly unknown; they are never assigned a guessed
//! meaning.

use core::fmt;
use dmc2_diagnostics::{DiagnosticMetadata, SelfDescribingDiagnostic};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VfdFaultFamily {
    Eoc,
    Eou,
    Elu,
    Eoh,
    Eol,
    Eoa,
    Eot,
}

impl VfdFaultFamily {
    pub const COUNT: usize = 7;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Eoc,
        Self::Eou,
        Self::Elu,
        Self::Eoh,
        Self::Eol,
        Self::Eoa,
        Self::Eot,
    ];

    pub const fn base_code(self) -> u32 {
        match self {
            Self::Eoc => 64,
            Self::Eou => 80,
            Self::Elu => 88,
            Self::Eoh => 92,
            Self::Eol => 96,
            Self::Eoa => 100,
            Self::Eot => 104,
        }
    }

    pub const fn display(self) -> &'static str {
        match self {
            Self::Eoc => "E.OC",
            Self::Eou => "E.oU",
            Self::Elu => "E.Lu",
            Self::Eoh => "E.oH",
            Self::Eol => "E.oL",
            Self::Eoa => "E.oA",
            Self::Eot => "E.oT",
        }
    }

    pub const fn hal_slug(self) -> &'static str {
        match self {
            Self::Eoc => "e-oc",
            Self::Eou => "e-ou",
            Self::Elu => "e-lu",
            Self::Eoh => "e-oh",
            Self::Eol => "e-ol",
            Self::Eoa => "e-oa",
            Self::Eot => "e-ot",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VfdFaultPhase {
    S,
    A,
    D,
    N,
}

impl VfdFaultPhase {
    pub const COUNT: usize = 4;
    pub const ALL: [Self; Self::COUNT] = [Self::S, Self::A, Self::D, Self::N];

    pub const fn offset(self) -> u32 {
        match self {
            Self::S => 0,
            Self::A => 1,
            Self::D => 2,
            Self::N => 3,
        }
    }

    pub const fn display(self) -> &'static str {
        match self {
            Self::S => "S",
            Self::A => "A",
            Self::D => "d",
            Self::N => "n",
        }
    }

    pub const fn hal_slug(self) -> &'static str {
        match self {
            Self::S => "s",
            Self::A => "a",
            Self::D => "d",
            Self::N => "n",
        }
    }

    const fn from_offset(offset: u32) -> Option<Self> {
        match offset {
            0 => Some(Self::S),
            1 => Some(Self::A),
            2 => Some(Self::D),
            3 => Some(Self::N),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VfdFaultCode {
    pub raw: u32,
    pub family: VfdFaultFamily,
    pub phase: VfdFaultPhase,
}

impl VfdFaultCode {
    pub const fn decode(raw: u32) -> Option<Self> {
        let mut index = 0;
        while index < VfdFaultFamily::COUNT {
            let family = VfdFaultFamily::ALL[index];
            let base = family.base_code();
            if raw >= base && raw <= base + 3 {
                let Some(phase) = VfdFaultPhase::from_offset(raw - base) else {
                    return None;
                };
                return Some(Self { raw, family, phase });
            }
            index += 1;
        }
        None
    }

    pub const fn name(self) -> &'static str {
        match (self.family, self.phase) {
            (VfdFaultFamily::Eoc, VfdFaultPhase::S) => "H100_E_OC_S",
            (VfdFaultFamily::Eoc, VfdFaultPhase::A) => "H100_E_OC_A",
            (VfdFaultFamily::Eoc, VfdFaultPhase::D) => "H100_E_OC_D",
            (VfdFaultFamily::Eoc, VfdFaultPhase::N) => "H100_E_OC_N",
            (VfdFaultFamily::Eou, VfdFaultPhase::S) => "H100_E_OU_S",
            (VfdFaultFamily::Eou, VfdFaultPhase::A) => "H100_E_OU_A",
            (VfdFaultFamily::Eou, VfdFaultPhase::D) => "H100_E_OU_D",
            (VfdFaultFamily::Eou, VfdFaultPhase::N) => "H100_E_OU_N",
            (VfdFaultFamily::Elu, VfdFaultPhase::S) => "H100_E_LU_S",
            (VfdFaultFamily::Elu, VfdFaultPhase::A) => "H100_E_LU_A",
            (VfdFaultFamily::Elu, VfdFaultPhase::D) => "H100_E_LU_D",
            (VfdFaultFamily::Elu, VfdFaultPhase::N) => "H100_E_LU_N",
            (VfdFaultFamily::Eoh, VfdFaultPhase::S) => "H100_E_OH_S",
            (VfdFaultFamily::Eoh, VfdFaultPhase::A) => "H100_E_OH_A",
            (VfdFaultFamily::Eoh, VfdFaultPhase::D) => "H100_E_OH_D",
            (VfdFaultFamily::Eoh, VfdFaultPhase::N) => "H100_E_OH_N",
            (VfdFaultFamily::Eol, VfdFaultPhase::S) => "H100_E_OL_S",
            (VfdFaultFamily::Eol, VfdFaultPhase::A) => "H100_E_OL_A",
            (VfdFaultFamily::Eol, VfdFaultPhase::D) => "H100_E_OL_D",
            (VfdFaultFamily::Eol, VfdFaultPhase::N) => "H100_E_OL_N",
            (VfdFaultFamily::Eoa, VfdFaultPhase::S) => "H100_E_OA_S",
            (VfdFaultFamily::Eoa, VfdFaultPhase::A) => "H100_E_OA_A",
            (VfdFaultFamily::Eoa, VfdFaultPhase::D) => "H100_E_OA_D",
            (VfdFaultFamily::Eoa, VfdFaultPhase::N) => "H100_E_OA_N",
            (VfdFaultFamily::Eot, VfdFaultPhase::S) => "H100_E_OT_S",
            (VfdFaultFamily::Eot, VfdFaultPhase::A) => "H100_E_OT_A",
            (VfdFaultFamily::Eot, VfdFaultPhase::D) => "H100_E_OT_D",
            (VfdFaultFamily::Eot, VfdFaultPhase::N) => "H100_E_OT_N",
        }
    }

    pub const fn hal_slug(self) -> &'static str {
        match (self.family, self.phase) {
            (VfdFaultFamily::Eoc, VfdFaultPhase::S) => "e-oc-s",
            (VfdFaultFamily::Eoc, VfdFaultPhase::A) => "e-oc-a",
            (VfdFaultFamily::Eoc, VfdFaultPhase::D) => "e-oc-d",
            (VfdFaultFamily::Eoc, VfdFaultPhase::N) => "e-oc-n",
            (VfdFaultFamily::Eou, VfdFaultPhase::S) => "e-ou-s",
            (VfdFaultFamily::Eou, VfdFaultPhase::A) => "e-ou-a",
            (VfdFaultFamily::Eou, VfdFaultPhase::D) => "e-ou-d",
            (VfdFaultFamily::Eou, VfdFaultPhase::N) => "e-ou-n",
            (VfdFaultFamily::Elu, VfdFaultPhase::S) => "e-lu-s",
            (VfdFaultFamily::Elu, VfdFaultPhase::A) => "e-lu-a",
            (VfdFaultFamily::Elu, VfdFaultPhase::D) => "e-lu-d",
            (VfdFaultFamily::Elu, VfdFaultPhase::N) => "e-lu-n",
            (VfdFaultFamily::Eoh, VfdFaultPhase::S) => "e-oh-s",
            (VfdFaultFamily::Eoh, VfdFaultPhase::A) => "e-oh-a",
            (VfdFaultFamily::Eoh, VfdFaultPhase::D) => "e-oh-d",
            (VfdFaultFamily::Eoh, VfdFaultPhase::N) => "e-oh-n",
            (VfdFaultFamily::Eol, VfdFaultPhase::S) => "e-ol-s",
            (VfdFaultFamily::Eol, VfdFaultPhase::A) => "e-ol-a",
            (VfdFaultFamily::Eol, VfdFaultPhase::D) => "e-ol-d",
            (VfdFaultFamily::Eol, VfdFaultPhase::N) => "e-ol-n",
            (VfdFaultFamily::Eoa, VfdFaultPhase::S) => "e-oa-s",
            (VfdFaultFamily::Eoa, VfdFaultPhase::A) => "e-oa-a",
            (VfdFaultFamily::Eoa, VfdFaultPhase::D) => "e-oa-d",
            (VfdFaultFamily::Eoa, VfdFaultPhase::N) => "e-oa-n",
            (VfdFaultFamily::Eot, VfdFaultPhase::S) => "e-ot-s",
            (VfdFaultFamily::Eot, VfdFaultPhase::A) => "e-ot-a",
            (VfdFaultFamily::Eot, VfdFaultPhase::D) => "e-ot-d",
            (VfdFaultFamily::Eot, VfdFaultPhase::N) => "e-ot-n",
        }
    }

    pub const fn summary(self) -> &'static str {
        match (self.family, self.phase) {
            (VfdFaultFamily::Eoc, VfdFaultPhase::S) => "the H100 detected over-current at stop",
            (VfdFaultFamily::Eoc, VfdFaultPhase::A) => {
                "the H100 detected over-current during acceleration"
            }
            (VfdFaultFamily::Eoc, VfdFaultPhase::D) => {
                "the H100 detected over-current during deceleration"
            }
            (VfdFaultFamily::Eoc, VfdFaultPhase::N) => {
                "the H100 detected over-current at constant speed"
            }
            (VfdFaultFamily::Eou, VfdFaultPhase::S) => "the H100 detected over-voltage at stop",
            (VfdFaultFamily::Eou, VfdFaultPhase::A) => {
                "the H100 detected over-voltage during acceleration"
            }
            (VfdFaultFamily::Eou, VfdFaultPhase::D) => {
                "the H100 detected over-voltage during deceleration"
            }
            (VfdFaultFamily::Eou, VfdFaultPhase::N) => {
                "the H100 detected over-voltage at constant speed"
            }
            (VfdFaultFamily::Elu, VfdFaultPhase::S) => {
                "the H100 detected low input voltage at stop"
            }
            (VfdFaultFamily::Elu, VfdFaultPhase::A) => {
                "the H100 detected low input voltage during acceleration"
            }
            (VfdFaultFamily::Elu, VfdFaultPhase::D) => {
                "the H100 detected low input voltage during deceleration"
            }
            (VfdFaultFamily::Elu, VfdFaultPhase::N) => {
                "the H100 detected low input voltage at constant speed"
            }
            (VfdFaultFamily::Eoh, VfdFaultPhase::S) => "the H100 inverter overheated at stop",
            (VfdFaultFamily::Eoh, VfdFaultPhase::A) => {
                "the H100 inverter overheated during acceleration"
            }
            (VfdFaultFamily::Eoh, VfdFaultPhase::D) => {
                "the H100 inverter overheated during deceleration"
            }
            (VfdFaultFamily::Eoh, VfdFaultPhase::N) => {
                "the H100 inverter overheated at constant speed"
            }
            (VfdFaultFamily::Eol, VfdFaultPhase::S) => {
                "the H100 inverter overload protection tripped at stop"
            }
            (VfdFaultFamily::Eol, VfdFaultPhase::A) => {
                "the H100 inverter overload protection tripped during acceleration"
            }
            (VfdFaultFamily::Eol, VfdFaultPhase::D) => {
                "the H100 inverter overload protection tripped during deceleration"
            }
            (VfdFaultFamily::Eol, VfdFaultPhase::N) => {
                "the H100 inverter overload protection tripped at constant speed"
            }
            (VfdFaultFamily::Eoa, VfdFaultPhase::S) => {
                "the H100 motor-overload protection tripped at stop"
            }
            (VfdFaultFamily::Eoa, VfdFaultPhase::A) => {
                "the H100 motor-overload protection tripped during acceleration"
            }
            (VfdFaultFamily::Eoa, VfdFaultPhase::D) => {
                "the H100 motor-overload protection tripped during deceleration"
            }
            (VfdFaultFamily::Eoa, VfdFaultPhase::N) => {
                "the H100 motor-overload protection tripped at constant speed"
            }
            (VfdFaultFamily::Eot, VfdFaultPhase::S) => {
                "the H100 detected motor over-torque at stop"
            }
            (VfdFaultFamily::Eot, VfdFaultPhase::A) => {
                "the H100 detected motor over-torque during acceleration"
            }
            (VfdFaultFamily::Eot, VfdFaultPhase::D) => {
                "the H100 detected motor over-torque during deceleration"
            }
            (VfdFaultFamily::Eot, VfdFaultPhase::N) => {
                "the H100 detected motor over-torque at constant speed"
            }
        }
    }

    pub const fn action(self) -> &'static str {
        match (self.family, self.phase) {
            (VfdFaultFamily::Eoc, VfdFaultPhase::A) => {
                "keep the spindle stopped; check motor/output wiring for shorts and insulation failure, check load and drive sizing, and lengthen acceleration before reset"
            }
            (VfdFaultFamily::Eoc, VfdFaultPhase::N) => {
                "keep the spindle stopped; check motor/output wiring, a blocked spindle or sudden load change, drive sizing, and supply-voltage changes before reset"
            }
            (VfdFaultFamily::Eoc, VfdFaultPhase::S | VfdFaultPhase::D) => {
                "keep the spindle stopped; check motor/output wiring for shorts and insulation failure, lengthen deceleration, and check drive sizing and DC-braking settings before reset"
            }
            (VfdFaultFamily::Eou, _) => {
                "keep the spindle stopped; check input voltage for abnormal changes and lengthen deceleration or verify the specified braking provision before reset"
            }
            (VfdFaultFamily::Elu, _) => {
                "keep the spindle stopped; verify input voltage, supply continuity, and any sudden load change before reset"
            }
            (VfdFaultFamily::Eoh, _) => {
                "keep the spindle stopped; clear blocked cooling airflow or fins, verify fan operation, ambient temperature, and ventilation, and allow the drive to cool before reset"
            }
            (VfdFaultFamily::Eol, _) => {
                "keep the spindle stopped; check for a jammed mechanical load, verify drive capacity, and correct the V/F configuration before reset"
            }
            (VfdFaultFamily::Eoa, _) => {
                "keep the spindle stopped; check for sudden or excessive mechanical load, verify motor sizing and condition, and inspect supply-voltage stability before reset"
            }
            (VfdFaultFamily::Eot, _) => {
                "keep the spindle stopped; inspect the mechanical load for a jam or sudden torque change and verify that the motor is correctly sized before reset"
            }
        }
    }
}

impl SelfDescribingDiagnostic for VfdFaultCode {
    fn metadata(self) -> DiagnosticMetadata {
        DiagnosticMetadata::new(
            self.raw as i64,
            self.name(),
            self.hal_slug(),
            self.summary(),
            self.action(),
        )
    }
}

impl fmt::Display for VfdFaultCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} (raw={}, drive_display={}.{}): {}; action: {}",
            self.name(),
            self.raw,
            self.family.display(),
            self.phase.display(),
            self.summary(),
            self.action(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_manual_listed_family_and_suffix_has_one_exact_raw_code() {
        let mut observed = 0;
        for family in VfdFaultFamily::ALL {
            for phase in VfdFaultPhase::ALL {
                let raw = family.base_code() + phase.offset();
                assert_eq!(
                    VfdFaultCode::decode(raw),
                    Some(VfdFaultCode { raw, family, phase })
                );
                let code = VfdFaultCode { raw, family, phase };
                assert!(code.metadata().complete());
                assert!(code.summary().starts_with("the H100"));
                assert!(code.action().starts_with("keep the spindle stopped;"));
                assert!(!code.action().contains("manual"));
                observed += 1;
            }
        }
        assert_eq!(observed, 28);
    }

    #[test]
    fn values_absent_from_the_manual_table_remain_unknown() {
        for raw in [0, 1, 63, 68, 79, 84, 87, 108, u32::MAX] {
            assert_eq!(VfdFaultCode::decode(raw), None, "raw={raw}");
        }
    }
}
