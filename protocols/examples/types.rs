// ============================================================================
// MATHILDE PROPRIETARY AND CONFIDENTIAL
// Copyright (c) 2024 MATHILDE. All Rights Reserved.
//
// This source code contains trade secrets and confidential information owned
// exclusively by MATHILDE, protected under Swiss law:
//
// - URG Art. 2(3), 10(3): Computer program copyright protection
// - URG Art. 24: Reverse engineering/decompilation restricted
// - UWG Art. 5-6: Trade secret and confidential information protection
// - StGB Art. 143bis: Unauthorized data access (criminal)
// - StGB Art. 162: Trade secret violation (criminal)
//
// PROHIBITED: Reproduction, copying, modification, distribution, disclosure,
// reverse engineering, decompilation, or derivative works without prior
// written authorization from MATHILDE.
//
// ACCESS REQUIREMENT: Executed NDA with MATHILDE required. Unauthorized
// access or possession violates Swiss law and international treaties.
//
// ALGORITHMS: Mathematical methods and parameters in this file constitute
// trade secrets independent of copyright protection.
//
// Legal Contact: massimo.nicora@wnlegal.ch
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetrendMethod {
    None,
    RemoveMean,
    RemoveLinear,
    RemovePolynomial { degree: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaveletFamily {
    ModwtD4,
    ModwtD6,
    ModwtD8,
    Haar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowFunction {
    Rectangular,
    Hann,
    Hamming,
    Blackman,
}
