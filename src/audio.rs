use glam::Vec2;
use glam::Vec3;
use std::sync::Arc;
use std::sync::Mutex;

//
const AUDIO_PACKET_SIZE: usize = 131072;

//
const AUDIO_DEVICE_FREQUENCY: i32 = 44_100;

//
const AUDIO_DEVICE_CHANNELS: u8 = 2;

#[derive(Clone, Debug)]
struct Source1D
{
  volume: f32,
}

#[derive(Clone, Debug)]
struct Source2D
{
  origin:        Vec2,
  velocity:      Vec2,
  volume_min:    f32,
  volume_max:    f32,
  distance_min:  f32,
  distance_max:  f32,
  sample_offset: usize,
}

#[derive(Clone, Debug)]
struct Source3D
{
  origin:        Vec3,
  velocity:      Vec3,
  volume_min:    f32,
  volume_max:    f32,
  distance_min:  f32,
  distance_max:  f32,
  sample_offset: usize,
}

#[derive(Clone, Debug)]
enum SourceInternal
{
  Source1D(Source1D),
  Source2D(Source2D),
  Source3D(Source3D),
}

#[derive(Clone, Debug)]
pub struct Source
{
  sample:   usize,
  internal: SourceInternal,
}

impl Source
{
  pub fn new() -> Arc<Mutex<Source>>
  {
    Arc::new(Mutex::new(Source {
      sample:   0,
      internal: SourceInternal::Source1D(Source1D {
        volume: 1.0f32
      }),
    }))
  }

  pub fn new_2d() -> Arc<Mutex<Source>>
  {
    Arc::new(Mutex::new(Source {
      sample:   0,
      internal: SourceInternal::Source2D(Source2D {
        origin:        Vec2::ZERO,
        velocity:      Vec2::ZERO,
        volume_min:    0.0f32,
        volume_max:    1.0f32,
        distance_min:  0.0f32,
        distance_max:  1.0f32,
        sample_offset: 0,
      }),
    }))
  }

  pub fn new_3d() -> Arc<Mutex<Source>>
  {
    Arc::new(Mutex::new(Source {
      sample:   0,
      internal: SourceInternal::Source3D(Source3D {
        origin:        Vec3::ZERO,
        velocity:      Vec3::ZERO,
        volume_min:    0.0f32,
        volume_max:    1.0f32,
        distance_min:  0.0f32,
        distance_max:  1.0f32,
        sample_offset: 0,
      }),
    }))
  }

  pub fn set_volume(&mut self, volume: f32)
  {
    // These invariants prevent clipping and potential damage
    // to the listener or audio equipment.
    assert!((0.0f32..=1.0f32).contains(&volume));

    if let SourceInternal::Source1D(internal) = &mut self.internal {
      internal.volume = volume;
    }
  }

  pub fn set_volume_clamp(&mut self, mut min: f32, mut max: f32)
  {
    // `volume_max` and `volume_min` must be within [0.0, 1.0] and
    // `volume_min` must be less-than `volume_max`. This prevents
    // clipping and potential damage to the listener or audio
    // equipment.
    assert!((0.0f32..=1.0f32).contains(&min));
    assert!((0.0f32..=1.0f32).contains(&max));
    assert!(min < max);

    match &mut self.internal {
      SourceInternal::Source2D(internal) => {
        internal.volume_min = min;
        internal.volume_max = max;
      }
      SourceInternal::Source3D(internal) => {
        internal.volume_min = min;
        internal.volume_max = max;
      }
      _ => {}
    };
  }

  pub fn set_distance_clamp(&mut self, mut min: f32, mut max: f32)
  {
    // `distance_max` and `distance_min` must be positive and
    // `distance_min` must be less-than `distance_max`.
    // Otherwise, the interpolation performed in `volume`will
    // compute an incorrect value.
    assert!(min >= 0.0f32);
    assert!(max >= 0.0f32);
    assert!(min < max);

    match &mut self.internal {
      SourceInternal::Source2D(internal) => {
        internal.distance_min = min;
        internal.distance_max = max;
      }
      SourceInternal::Source3D(internal) => {
        internal.distance_min = min;
        internal.distance_max = max;
      }
      _ => {}
    };
  }

  pub fn set_origin_2d(&mut self, origin: Vec2)
  {
    if let SourceInternal::Source2D(internal) = &mut self.internal {
      internal.origin = origin;

      // `sample_offset` must be updated to the current `sample` when
      // `origin` is modified. `sample_offset` reduces the quantity
      // of samples elapsed for the calculation in `volume`. This
      // offset required so the sound is emitted from the
      // correct origin.
      internal.sample_offset = self.sample;
    }
  }

  pub fn set_origin_3d(&mut self, origin: Vec3)
  {
    if let SourceInternal::Source3D(internal) = &mut self.internal {
      internal.origin = origin;

      // `sample_offset` must be updated to the current `sample` when
      // `origin` is modified. `sample_offset` reduces the quantity
      // of samples elapsed for the calculation in `volume`. This
      // offset required so the sound is emitted from the
      // correct origin.
      internal.sample_offset = self.sample;
    }
  }

  pub fn set_velocity_2d(&mut self, velocity: Vec2)
  {
    self.set_origin_2d(
      #[rustfmt::skip]
      if let SourceInternal::Source2D(internal) = &self.internal {
        // Computes the number of seconds that has `elapsed`. Updating
        // `velocity` without also updating `origin`, will cause the
        // sound to be emitted from an unexpected origin.
        let elapsed = self.sample as f32 / (
          AUDIO_DEVICE_CHANNELS as f32 *
          AUDIO_DEVICE_FREQUENCY as f32
        );

        internal.origin + (internal.velocity * elapsed)
      } else {
        // `set_origin_2d` does nothing if `internal` is not of type
        // `Source2D`. Therefore, it is safe to pass a zero-vector
        // to `set_origin_2d`.
        Vec2::ZERO
      },
    );

    if let SourceInternal::Source2D(internal) = &mut self.internal {
      internal.velocity = velocity;
    }
  }

  pub fn set_velocity_3d(&mut self, velocity: Vec3)
  {
    self.set_origin_3d(
      #[rustfmt::skip]
      if let SourceInternal::Source3D(internal) = &self.internal {
        // Computes the number of seconds that has `elapsed`. Updating
        // `velocity` without also updating `origin`, will cause the
        // sound to be emitted from an unexpected origin.
        let elapsed = self.sample as f32 / (
          AUDIO_DEVICE_CHANNELS as f32 *
          AUDIO_DEVICE_FREQUENCY as f32
        );

        internal.origin + (internal.velocity * elapsed)
      } else {
        // `set_origin_3d` does nothing if `internal` is not of type
        // `Source3D`. Therefore, it is safe to pass a zero-vector
        // to `set_origin_3d`.
        Vec3::ZERO
      },
    );

    if let SourceInternal::Source3D(internal) = &mut self.internal {
      internal.velocity = velocity;
    }
  }

  pub fn volume(&mut self, sample: usize, channels: usize) -> f32
  {
    fn linear(r: f32) -> f32
    {
      1.0f32 - r.clamp(0.0f32, 1.0f32)
    }

    #[rustfmt::skip]
    match &mut self.internal {
      SourceInternal::Source1D(internal) => {
        internal.volume
      }

      SourceInternal::Source3D(_) => {
        1.0 // TODO(bnpfeife)
      }

      SourceInternal::Source2D(internal) => {
        // Computing the seconds `elapsed` since `sample_offset` allows
        // the caller to mutate `origin` and `velocity` and have the
        // source be positioned in an "expected" fashion.
        let elapsed = (sample - internal.sample_offset) as f32 / (
          AUDIO_DEVICE_CHANNELS as f32 *
          AUDIO_DEVICE_FREQUENCY as f32
        );

        let position = internal.origin + (internal.velocity * elapsed);

        let angle = {
          let angle = match channels {
            0 => -Vec2::X, // NX "left" listener
            _ =>  Vec2::X, // PX "right" listener
          }
          .dot(
            // If the position is a zero-vector, it cannot be normalized into
            // a unit-vector. This produces an audible artifact when sources
            // overlap the listener. To mitigate this, if a source overlaps
            // the listener, NX and PX are played at equal volumes.
            if position != Vec2::ZERO { position.normalize() } else { Vec2::Y }
          );
          // normalize the dot-product to [0.0, 1.0]
          (angle + 1.0f32) / 2.0f32
        };

        let distance = position.length().abs();
        if distance <= internal.distance_min {
          return internal.volume_max;
        }
        if distance >= internal.distance_max {
          return internal.volume_min;
        }
        (
          angle * (
            (internal.volume_max - internal.volume_min) *
              linear(
                // The `*_gain` functions require that the distance
                // is between [0.0, 1.0]. Since this computation is
                // performed beetween `distance_min/max`, the
                // result is always between [0.0, 1.0].
                (         distance     - internal.distance_min) /
                (internal.distance_max - internal.distance_min)
              )
          ) + internal.volume_min
        ).clamp(
          internal.volume_min,
          internal.volume_max
        )
      }
    }
  }
}
