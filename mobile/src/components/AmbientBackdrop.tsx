import React from 'react';
import { Image, StyleSheet, View } from 'react-native';
import { BlurView } from 'expo-blur';
import { LinearGradient } from 'expo-linear-gradient';
import { usePlayer } from '../context/PlayerContext';

export function AmbientBackdrop({ children }: { children: React.ReactNode }) {
  const { currentTrack, palette, dynamicThemeEnabled } = usePlayer();
  const artworkUri = dynamicThemeEnabled ? currentTrack?.artwork_ref : null;

  return (
    <View style={styles.root}>
      {/* Base deep obsidian canvas */}
      <LinearGradient
        colors={[palette.bg_top, palette.bg_mid, '#07090F']}
        locations={[0, 0.48, 1]}
        style={StyleSheet.absoluteFill}
      />

      {/* Desktop-style luminous blurred album art layer */}
      {artworkUri ? (
        <>
          <Image
            source={{ uri: artworkUri }}
            style={styles.artworkBackdrop}
            blurRadius={90}
            resizeMode="cover"
          />
          <BlurView intensity={55} tint="dark" style={StyleSheet.absoluteFill} />
        </>
      ) : (
        <LinearGradient
          colors={['rgba(99, 102, 241, 0.20)', 'rgba(139, 92, 246, 0.08)', 'transparent']}
          locations={[0, 0.42, 0.9]}
          style={StyleSheet.absoluteFill}
        />
      )}

      {/* Smooth dark crystal scrim so text & glass cards stay razor-sharp */}
      <LinearGradient
        colors={[
          'rgba(8, 10, 16, 0.46)',
          'rgba(8, 10, 16, 0.72)',
          'rgba(7, 9, 14, 0.94)',
        ]}
        locations={[0, 0.52, 1]}
        style={StyleSheet.absoluteFill}
      />

      {children}
    </View>
  );
}

const styles = StyleSheet.create({
  root: {
    flex: 1,
    backgroundColor: '#07090F',
  },
  artworkBackdrop: {
    position: 'absolute',
    top: 0,
    right: 0,
    bottom: 0,
    left: 0,
    opacity: 0.48,
    transform: [{ scale: 1.18 }],
  },
});
