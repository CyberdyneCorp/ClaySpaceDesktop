//! A scalar stamp, and where one may be used.
//!
//! An alpha is a greyscale image read as a height offset: pores, fabric,
//! scales, stitching — the detail work every sculptor does with a stamp rather
//! than a shape. The engine decodes no images, which is deliberate on its part
//! (an image decoder in a library that compiles to five backends is a
//! liability), so loading one is the application's and this is what the
//! application loads it into.

/// A greyscale stamp, ready for the engine.
///
/// Samples are row-major, `width * height` of them, each in 0..=1. Nothing
/// here remembers where it came from beyond a name: the file is read once and
/// the pixels are what is kept.
#[derive(Debug, Clone, PartialEq)]
pub struct Alpha {
    /// What the interface calls it. The file's stem.
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// `width * height` scalars, row-major, 0..=1.
    pub samples: Vec<f32>,
}

impl Alpha {
    /// The smallest stamp that is a stamp.
    ///
    /// Below two in either direction there is nothing to interpolate between,
    /// and the engine refuses it — so it is refused here, where the reason can
    /// be stated in a sentence rather than arriving as an error code.
    pub const MIN_SIDE: u32 = 2;

    /// The largest stamp accepted, per side.
    ///
    /// A stamp is sampled per evaluation, and one this size is already four
    /// megapixels of scalars — sixteen megabytes. Past it the cost is not the
    /// memory but the cache: the sampling is random-access and a stamp that
    /// does not fit reads from main memory on every lookup.
    pub const MAX_SIDE: u32 = 2048;

    /// Checks the shape, so a malformed stamp is refused before it is used.
    pub fn validated(self) -> Result<Self, AlphaRefusal> {
        if self.width < Self::MIN_SIDE || self.height < Self::MIN_SIDE {
            return Err(AlphaRefusal::TooSmall {
                width: self.width,
                height: self.height,
            });
        }
        if self.width > Self::MAX_SIDE || self.height > Self::MAX_SIDE {
            return Err(AlphaRefusal::TooLarge {
                width: self.width,
                height: self.height,
            });
        }
        let expected = self.width as usize * self.height as usize;
        if self.samples.len() != expected {
            return Err(AlphaRefusal::Truncated {
                expected,
                found: self.samples.len(),
            });
        }
        Ok(self)
    }
}

/// Why a file could not become a stamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlphaRefusal {
    /// Not a PNG. Stated by name rather than left to a decoder error naming a
    /// library the sculptor has never heard of.
    NotPng {
        extension: String,
    },
    /// The file could not be read or decoded.
    Unreadable(String),
    TooSmall {
        width: u32,
        height: u32,
    },
    TooLarge {
        width: u32,
        height: u32,
    },
    /// The pixels do not fill the dimensions the header claims.
    Truncated {
        expected: usize,
        found: usize,
    },
}

impl std::fmt::Display for AlphaRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotPng { extension } if extension.is_empty() => {
                f.write_str("os alfas são lidos apenas em PNG; este arquivo não tem extensão")
            }
            Self::NotPng { extension } => write!(
                f,
                "os alfas são lidos apenas em PNG; este é um {}",
                extension.to_uppercase()
            ),
            Self::Unreadable(why) => write!(f, "o PNG não pôde ser lido: {why}"),
            Self::TooSmall { width, height } => write!(
                f,
                "um alfa de {width}×{height} não tem entre o que interpolar; \
                 o mínimo é {min}×{min}",
                min = Alpha::MIN_SIDE
            ),
            Self::TooLarge { width, height } => write!(
                f,
                "um alfa de {width}×{height} passa do limite de {max}×{max}",
                max = Alpha::MAX_SIDE
            ),
            Self::Truncated { expected, found } => {
                write!(f, "o PNG declara {expected} amostras e traz {found}")
            }
        }
    }
}

/// Where an alpha stamp is accepted.
///
/// Not every representation takes one, and the two that do take it by
/// different routes — a field gets a deformer appended to the item, a grid gets
/// a carve modulated per cell. A mesh takes one too, through the same brush
/// block the other mesh verbs use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlphaSupport {
    /// Offered, and this is how it reaches the engine.
    Accepted,
    /// The representation has no route for one, and saying so is better than
    /// a control that appears to work.
    NotHere {
        representation: crate::Representation,
    },
    /// The operation chosen has no surface to modulate.
    NotWithThisOperation { op: crate::Combine },
    /// A field takes an alpha on a *placed item*, and a stroke does not place
    /// one — it hands the engine a template.
    NotThroughAStroke,
}

impl AlphaSupport {
    /// Whether an alpha may be used on this representation with this
    /// operation.
    ///
    /// An alpha offsets a distance along the surface's own normal. That needs
    /// a surface already there to modulate — which is what the engine's own
    /// note says about why it is a deformer rather than a primitive — so the
    /// operations that build a shape rather than displace one have nothing for
    /// it to modulate.
    pub fn of(representation: crate::Representation, op: crate::Combine) -> Self {
        if representation != crate::Representation::Sdf {
            return Self::Accepted;
        }
        // A field's alpha is a deformer on an item, and `clay_layer_apply_stroke`
        // uses its item as a *template scaled to each stamp's radius* — the
        // deformer chain does not travel with it. Measured at the engine
        // boundary and recorded in `claycore/tests/alpha_deformer.rs`: the same
        // stroke with an alpha of amplitude 0, 0.05 and 0.25 produces a surface
        // identical to four decimal places under both Add and Relief, while the
        // same alpha on an item placed with `add_item` changes it and grades
        // with the amplitude.
        //
        // So the refusal is stated before the operation is looked at. Checking
        // the operation first would give a sculptor the more specific-sounding
        // "that operation has nothing to modulate" for a control that would not
        // have worked under any operation.
        if !op.displaces_along_the_normal() {
            // Kept, because it is the reason the day the stroke carries a
            // chain: even then, an operation that builds a shape has no
            // surface for a stamp to modulate.
            let _ = Self::NotWithThisOperation { op };
        }
        Self::NotThroughAStroke
    }

    pub fn accepted(self) -> bool {
        self == Self::Accepted
    }
}

impl std::fmt::Display for AlphaSupport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Accepted => f.write_str("aceito"),
            Self::NotHere { representation } => write!(
                f,
                "um alfa não se aplica a uma camada {}",
                representation.label()
            ),
            Self::NotWithThisOperation { op } => write!(
                f,
                "um alfa modula uma superfície que já existe; {} constrói uma \
                 forma em vez de deslocar a que está lá",
                op.label()
            ),
            Self::NotThroughAStroke => f.write_str(
                "o motor aplica um alfa a um item colocado, e uma pincelada \
                 não coloca um: ela entrega um modelo que é reescalado a cada \
                 marca, sem a cadeia de deformadores",
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Combine, Representation};

    fn stamp(width: u32, height: u32) -> Alpha {
        Alpha {
            name: "teste".into(),
            width,
            height,
            samples: vec![0.5; width as usize * height as usize],
        }
    }

    #[test]
    fn a_well_formed_stamp_is_accepted() {
        assert!(stamp(64, 64).validated().is_ok());
    }

    /// One pixel across has nothing to interpolate between, which is the
    /// engine's own refusal — stated here so the sculptor gets a sentence
    /// rather than an error code.
    #[test]
    fn a_stamp_too_small_to_interpolate_is_refused_by_size() {
        let error = stamp(1, 64).validated().expect_err("one pixel across");
        assert!(
            error.to_string().contains('1'),
            "the refusal has to name the size it got: {error}"
        );
    }

    #[test]
    fn a_stamp_past_the_limit_is_refused_with_the_limit() {
        let alpha = Alpha {
            name: "grande".into(),
            width: Alpha::MAX_SIDE + 1,
            height: 4,
            // Not actually allocated: the shape is refused before the samples
            // are looked at, which is the point of checking dimensions first.
            samples: Vec::new(),
        };
        let error = alpha.validated().expect_err("past the limit");
        assert!(error.to_string().contains(&Alpha::MAX_SIDE.to_string()));
    }

    /// A header that claims more than the file holds is the shape a malformed
    /// image takes, and reading past the samples is what it would cost.
    #[test]
    fn a_stamp_whose_pixels_do_not_fill_it_is_refused() {
        let alpha = Alpha {
            name: "curto".into(),
            width: 8,
            height: 8,
            samples: vec![0.0; 10],
        };
        let error = alpha.validated().expect_err("ten samples for sixty-four");
        assert!(matches!(error, AlphaRefusal::Truncated { .. }), "{error}");
    }

    #[test]
    fn a_file_that_is_not_a_png_is_refused_by_name() {
        let error = AlphaRefusal::NotPng {
            extension: "jpg".into(),
        };
        assert!(error.to_string().contains("JPG"), "{error}");
        assert!(error.to_string().contains("PNG"), "{error}");
    }

    /// A stroke on a field takes no stamp, whatever the operation.
    ///
    /// The engine's own limit rather than a choice: the stroke's item is a
    /// template and its deformer chain is not carried. Measured in
    /// `claycore/tests/alpha_deformer.rs`, which is where this changes back if
    /// the engine ever carries one.
    #[test]
    fn a_field_stroke_takes_no_stamp_whatever_the_operation() {
        for op in Combine::ALL {
            let support = AlphaSupport::of(Representation::Sdf, op);
            assert!(
                !support.accepted(),
                "{} was offered a stamp a stroke cannot carry",
                op.label()
            );
            assert_eq!(support, AlphaSupport::NotThroughAStroke);
        }
    }

    /// And the refusal says which of the two reasons it is, so a sculptor is
    /// not told a control is unavailable with no way to know whether that is
    /// permanent.
    #[test]
    fn the_refusal_names_the_stroke_rather_than_the_representation() {
        let message = AlphaSupport::of(Representation::Sdf, Combine::Relief).to_string();
        assert!(
            message.contains("pincelada"),
            "the refusal blames the representation rather than the stroke: {message}"
        );
    }

    /// Cells and vertices take a stamp by their own routes — a carve of their
    /// own for a grid, a block in the brush descriptor for a mesh — so neither
    /// is gated on the SDF combine operation, and both work.
    #[test]
    fn the_other_representations_take_a_stamp() {
        for representation in [Representation::Voxel, Representation::Mesh] {
            for op in Combine::ALL {
                assert!(
                    AlphaSupport::of(representation, op).accepted(),
                    "{} was refused a stamp on a {} layer",
                    op.label(),
                    representation.label()
                );
            }
        }
    }
}
