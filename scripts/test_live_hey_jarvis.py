"""Campaign stop policy only: no application, audio, fixtures or models."""
import unittest
from live_hey_jarvis import stop_reason


def status(count, state, background):
    return {'state': state, 'background': {'activations': count, 'state': background}}


class CampaignStopTests(unittest.TestCase):
    def test_initial_off_does_not_end_trial(self):
        self.assertIsNone(stop_reason(status(0, 'idle', 'off')))

    def test_second_turn_can_finish_decoding(self):
        for state in ('recording', 'transcribing'):
            self.assertIsNone(stop_reason(status(2, state, 'busy')))

    def test_second_activation_ends_trial_even_if_first_had_no_draft(self):
        self.assertIsNotNone(stop_reason(status(2, 'transcript_ready', 'rearming')))
        self.assertIsNotNone(stop_reason(status(2, 'idle', 'listening')))

    def test_pause_cannot_be_followed_by_another_resume_in_campaign(self):
        for count in (1, 2):
            self.assertIsNotNone(stop_reason(status(count, 'idle', 'paused')))

    def test_error_and_excess_count_fail_closed(self):
        self.assertIsNotNone(stop_reason(status(0, 'idle', 'error')))
        self.assertEqual(stop_reason(status(3, 'recording', 'busy')),
                         'Activation count exceeded consent')


if __name__ == '__main__':
    unittest.main()
