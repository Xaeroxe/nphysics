use na::{DVector, Real, Unit};
use std::ops::Range;

use joint::JointConstraint;
use math::{AngularVector, Point, Vector, DIM, SPATIAL_DIM};
use object::{BodyHandle, BodySet};
use solver::helper;
use solver::{ConstraintSet, GenericNonlinearConstraint, IntegrationParameters,
             NonlinearConstraintGenerator};

/// A constraint that removes two translational and two rotational degrees of freedoms.
///
/// This is different frmo the cylindrical constraint since the remaining rotation and translation
/// are not restricted to be done wrt. the same axis.
pub struct PinSlotConstraint<N: Real> {
    b1: BodyHandle,
    b2: BodyHandle,
    anchor1: Point<N>,
    anchor2: Point<N>,
    axis_v1: Unit<Vector<N>>,
    axis_w1: Unit<Vector<N>>,
    axis_w2: Unit<Vector<N>>,
    lin_impulses: Vector<N>,
    ang_impulses: AngularVector<N>,
    bilateral_ground_rng: Range<usize>,
    bilateral_rng: Range<usize>,
    // min_offset: Option<N>,
    // max_offset: Option<N>,
}

impl<N: Real> PinSlotConstraint<N> {
    /// Creates a new pin-slot constraint.
    ///
    /// This will ensure the relative linear motions are always along `axis_v1` (here expressed
    /// in the local coordinate frame of `b1`), and that `axis_w1` and `axis_w2` always coincide.
    /// All axises and anchors are expressed in the local coordinate frame of their respective body
    /// part.
    pub fn new(
        b1: BodyHandle,
        b2: BodyHandle,
        anchor1: Point<N>,
        axis_v1: Unit<Vector<N>>,
        axis_w1: Unit<Vector<N>>,
        anchor2: Point<N>,
        axis_w2: Unit<Vector<N>>,
    ) -> Self {
        // let min_offset = None;
        // let max_offset = None;

        PinSlotConstraint {
            b1,
            b2,
            anchor1,
            anchor2,
            axis_v1,
            axis_w1,
            axis_w2,
            lin_impulses: Vector::zeros(),
            ang_impulses: AngularVector::zeros(),
            bilateral_ground_rng: 0..0,
            bilateral_rng: 0..0,
            // min_offset,
            // max_offset,
        }
    }

    // pub fn min_offset(&self) -> Option<N> {
    //     self.min_offset
    // }

    // pub fn max_offset(&self) -> Option<N> {
    //     self.max_offset
    // }

    // pub fn disable_min_offset(&mut self) {
    //     self.min_offset = None;
    // }

    // pub fn disable_max_offset(&mut self) {
    //     self.max_offset = None;
    // }

    // pub fn enable_min_offset(&mut self, limit: N) {
    //     self.min_offset = Some(limit);
    //     self.assert_limits();
    // }

    // pub fn enable_max_offset(&mut self, limit: N) {
    //     self.max_offset = Some(limit);
    //     self.assert_limits();
    // }

    // fn assert_limits(&self) {
    //     if let (Some(min_offset), Some(max_offset)) = (self.min_offset, self.max_offset) {
    //         assert!(
    //             min_offset <= max_offset,
    //             "RevoluteJoint constraint limits: the min angle must be larger than (or equal to) the max angle.");
    //     }
    // }
}

impl<N: Real> JointConstraint<N> for PinSlotConstraint<N> {
    fn num_velocity_constraints(&self) -> usize {
        SPATIAL_DIM - 2
    }

    fn anchors(&self) -> (BodyHandle, BodyHandle) {
        (self.b1, self.b2)
    }

    fn velocity_constraints(
        &mut self,
        _: &IntegrationParameters<N>,
        bodies: &BodySet<N>,
        ext_vels: &DVector<N>,
        ground_j_id: &mut usize,
        j_id: &mut usize,
        jacobians: &mut [N],
        constraints: &mut ConstraintSet<N>,
    ) {
        let b1 = bodies.body_part(self.b1);
        let b2 = bodies.body_part(self.b2);

        /*
         *
         * Joint constraints.
         *
         */
        let pos1 = b1.position();
        let pos2 = b2.position();

        let anchor1 = pos1 * self.anchor1;
        let anchor2 = pos2 * self.anchor2;

        let assembly_id1 = b1.parent_companion_id();
        let assembly_id2 = b2.parent_companion_id();

        let first_bilateral_ground = constraints.velocity.bilateral_ground.len();
        let first_bilateral = constraints.velocity.bilateral.len();

        let axis_v1 = pos1 * self.axis_v1;
        let axis_w1 = pos1 * self.axis_w1;

        helper::restrict_relative_linear_velocity_to_axis(
            &b1,
            &b2,
            assembly_id1,
            assembly_id2,
            &anchor1,
            &anchor2,
            &axis_v1,
            ext_vels,
            self.lin_impulses.as_slice(),
            0,
            ground_j_id,
            j_id,
            jacobians,
            constraints,
        );

        helper::restrict_relative_angular_velocity_to_axis(
            &b1,
            &b2,
            assembly_id1,
            assembly_id2,
            &axis_w1,
            &anchor1,
            &anchor2,
            ext_vels,
            self.ang_impulses.as_slice(),
            DIM - 1,
            ground_j_id,
            j_id,
            jacobians,
            constraints,
        );

        /*
         *
         * Limit constraints.
         *
         */

        self.bilateral_ground_rng =
            first_bilateral_ground..constraints.velocity.bilateral_ground.len();
        self.bilateral_rng = first_bilateral..constraints.velocity.bilateral.len();
    }

    fn cache_impulses(&mut self, constraints: &ConstraintSet<N>) {
        for c in &constraints.velocity.bilateral_ground[self.bilateral_ground_rng.clone()] {
            if c.impulse_id < DIM {
                self.lin_impulses[c.impulse_id] = c.impulse;
            } else {
                self.ang_impulses[c.impulse_id - DIM] = c.impulse;
            }
        }

        for c in &constraints.velocity.bilateral[self.bilateral_rng.clone()] {
            if c.impulse_id < DIM {
                self.lin_impulses[c.impulse_id] = c.impulse;
            } else {
                self.ang_impulses[c.impulse_id - DIM] = c.impulse;
            }
        }
    }
}

impl<N: Real> NonlinearConstraintGenerator<N> for PinSlotConstraint<N> {
    fn num_position_constraints(&self, bodies: &BodySet<N>) -> usize {
        // FIXME: calling this at each iteration of the non-linear resolution is costly.
        if self.is_active(bodies) {
            2
        } else {
            0
        }
    }

    fn position_constraint(
        &self,
        params: &IntegrationParameters<N>,
        i: usize,
        bodies: &mut BodySet<N>,
        jacobians: &mut [N],
    ) -> Option<GenericNonlinearConstraint<N>> {
        let body1 = bodies.body_part(self.b1);
        let body2 = bodies.body_part(self.b2);

        let pos1 = body1.position();
        let pos2 = body2.position();

        let anchor1 = pos1 * self.anchor1;
        let anchor2 = pos2 * self.anchor2;

        if i == 0 {
            let axis1 = pos1 * self.axis_w1;
            let axis2 = pos2 * self.axis_w2;

            return helper::align_axis(
                params,
                &body1,
                &body2,
                &anchor1,
                &anchor2,
                &axis1,
                &axis2,
                jacobians,
            );
        }

        if i == 1 {
            let axis = pos1 * self.axis_v1;

            return helper::project_anchor_to_axis(
                params,
                &body1,
                &body2,
                &anchor1,
                &anchor2,
                &axis,
                jacobians,
            );
        }

        return None;
    }
}
