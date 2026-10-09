"""Independent exact assertions for the finite cycle 0012 witnesses."""

import unittest
from dataclasses import replace
from fractions import Fraction as Q

from baseline import ScalarGaussian
from calibration_contracts import (
    Calibration,
    Record,
    drift_joint,
    drift_reference,
    evidence,
    field_witness,
    inverse,
    learned_calibration,
    select_survivors,
    selector_records,
    target_from_calibration,
    target_joint,
    target_reference,
    use_derivative,
    validated,
)


class CalibrationContractsTests(unittest.TestCase):
    def test_all_declared_exact_models_match_observation_space_reference(self):
        cases = evidence()["drift_cases"]
        self.assertEqual(len(cases), 32)
        for case in cases:
            with self.subTest(model=case["model"], values=case["observations"]):
                self.assertEqual(case["joint"]["x"], case["reference"])
                self.assertNotEqual(
                    case["joint"]["x"].variance, case["bad_independent"].variance
                )
                self.assertNotEqual(
                    case["joint"]["x"].variance, case["bad_no_drift"].variance
                )

    def test_main_drift_and_full_cross_covariance(self):
        result = drift_joint((0, 6), Calibration())
        self.assertEqual(result["x"], ScalarGaussian(Q(2), Q(14, 3)))
        self.assertEqual(result["mean_x_b0_d"], (Q(2), Q(0), Q(2)))
        # Independent derivation: b0 is not informed by these targets with flat x.
        # Difference z1-z0 informs d with likelihood variance 2 and prior variance 1.
        cov = result["covariance_x_b0_d"]
        self.assertEqual(cov[1][1], Q(4))
        self.assertEqual(cov[2][2], Q(2, 3))
        self.assertEqual(cov[0][1], Q(-4))
        self.assertEqual(cov[1][2], Q(0))
        self.assertEqual(cov[0][2], Q(-1, 3))

    def test_collapsed_state_cannot_determine_drift_replacement(self):
        old, new = Calibration(), Calibration(revision=2, drift=4)
        a, b = (0, 6), (1, 4)
        self.assertEqual(drift_joint(a, old)["x"], drift_joint(b, old)["x"])
        self.assertEqual(drift_joint(a, new)["x"], ScalarGaussian(Q(1), Q(29, 6)))
        self.assertEqual(drift_joint(b, new)["x"], ScalarGaussian(Q(3, 2), Q(29, 6)))

    def test_model_replacement_is_not_an_extra_independent_prior(self):
        replacement = drift_reference((0, 6), Calibration(revision=2, drift=4))
        appended = drift_joint((0, 6), Calibration(revision=2, drift=Q(4, 5)))["x"]
        self.assertNotEqual(appended, replacement)
        self.assertLess(appended.variance, replacement.variance)

    def test_gain_revision_requires_rebuilding_coefficients(self):
        old = drift_joint((0, 6), Calibration())["x"]
        model = Calibration(revision=2, gain=2)
        new = drift_joint((0, 6), model)["x"]
        self.assertEqual(new, ScalarGaussian(Q(1), Q(7, 6)))
        self.assertEqual(new, drift_reference((0, 6), model))
        self.assertNotEqual(old, new)

    def test_identity_change_cannot_silently_reinterpret_old_data(self):
        with self.assertRaises(ValueError):
            drift_joint((0, 6), Calibration(identity="different-hardware"))

    def test_calibration_training_propagates_to_target_result(self):
        records = selector_records()
        self.assertEqual(target_joint(records), ScalarGaussian(Q(6, 5), Q(17, 15)))
        self.assertEqual(target_joint(records), target_reference(records))
        calibration = learned_calibration(records)
        self.assertEqual(calibration.support, ("c10@1:calibration-offset",))
        self.assertEqual((calibration.mean, calibration.variance), (Q(24, 5), Q(4, 5)))

    def test_sensor_withdrawal_rebuilds_shared_calibration(self):
        records = select_survivors(selector_records(), sensor="A")
        self.assertEqual([r.record_id for r in records], ["b10"])
        self.assertEqual(target_joint(records), ScalarGaussian(Q(6), Q(5)))
        self.assertEqual(target_joint(records), target_reference(records))

    def test_time_boundary_uses_original_acquisition_time(self):
        records = select_survivors(selector_records(), interval=(10, 11))
        self.assertEqual([r.time for r in records], [9, 11])
        self.assertEqual(target_joint(records), ScalarGaussian(Q(6), Q(9, 2)))
        self.assertEqual(target_joint(records), target_reference(records))

    def test_field_and_conjunction_withdraw_training_not_target_records(self):
        records = selector_records()
        by_field = select_survivors(records, field="calibration-offset")
        conjunction = select_survivors(
            records, sensor="A", interval=(10, 11), field="calibration-offset"
        )
        self.assertEqual(by_field, conjunction)
        self.assertEqual(len(by_field), 3)
        self.assertEqual(target_joint(by_field), ScalarGaussian(Q(6), Q(13, 3)))
        self.assertEqual(target_joint(by_field), target_reference(by_field))

    def test_policy_split_requires_explicit_derivative_permission(self):
        records = selector_records()
        artifact = learned_calibration(records)
        targets = tuple(r for r in records if r.field == "signal")
        self.assertEqual(
            use_derivative(targets, artifact, artifact.support), target_joint(records)
        )
        self.assertIsNone(use_derivative(targets, artifact, ()))
        self.assertIsNone(use_derivative((), artifact, ()))
        self.assertEqual(
            use_derivative(targets, artifact, (), explicit_p2_grant=True),
            target_joint(records),
        )
        self.assertNotEqual(
            target_from_calibration(targets, artifact), target_joint(targets)
        )

    def test_withdrawn_training_value_does_not_affect_rebuilt_answer(self):
        original = selector_records()
        changed = tuple(
            replace(r, value=Q(-100)) if r.field == "calibration-offset" else r
            for r in original
        )
        self.assertNotEqual(target_joint(original), target_joint(changed))
        survivors = [
            select_survivors(rs, field="calibration-offset")
            for rs in (original, changed)
        ]
        self.assertEqual(target_joint(survivors[0]), target_joint(survivors[1]))

    def test_correlated_field_marginalization_not_precision_slicing(self):
        result = field_witness()
        self.assertEqual(result["full"], ScalarGaussian(Q(2), Q(3, 4)))
        self.assertEqual(result["after_withdraw_v"], ScalarGaussian(Q(0), Q(1)))
        self.assertEqual(
            result["bad_precision_and_information_slice"],
            ScalarGaussian(Q(-2), Q(3, 4)),
        )
        self.assertEqual(
            result["bad_conditional_precision_with_only_u"],
            ScalarGaussian(Q(0), Q(3, 4)),
        )

    def test_withdrawn_field_value_cannot_affect_surviving_likelihood(self):
        first, second = field_witness(v=Q(4)), field_witness(v=Q(-100))
        self.assertEqual(first["after_withdraw_v"], second["after_withdraw_v"])
        self.assertNotEqual(
            first["bad_precision_and_information_slice"],
            second["bad_precision_and_information_slice"],
        )

    def test_duplicate_identity_and_invalid_selector_are_rejected(self):
        records = selector_records()
        for duplicate in (records[0], replace(records[0], value=Q(20))):
            with self.assertRaises(ValueError):
                validated((*records, duplicate))
        for interval in ((11, 10), (10, 10), (10.0, 11), (10,)):
            with self.assertRaises(ValueError):
                select_survivors(records, interval=interval)
        with self.assertRaises(ValueError):
            select_survivors(records)
        with self.assertRaises(ValueError):
            target_joint([records[-1]])

    def test_finite_bounds_and_model_checks(self):
        with self.assertRaises(ValueError):
            inverse([[1] * 4] * 4)
        with self.assertRaises(ValueError):
            inverse(((1, 1), (1, 1)))
        with self.assertRaises(ValueError):
            validated(Record(str(i), "A", i, "signal", Q(0)) for i in range(9))
        for model in ({"drift": 0}, {"tau": -1}, {"gain": 0}, {"revision": 0}):
            with self.assertRaises(ValueError):
                Calibration(**model)

    def test_order_of_retained_evidence_does_not_change_answer(self):
        records = selector_records()
        self.assertEqual(target_joint(records), target_joint(tuple(reversed(records))))
        self.assertEqual(
            learned_calibration(records), learned_calibration(tuple(reversed(records)))
        )


if __name__ == "__main__":
    unittest.main()
