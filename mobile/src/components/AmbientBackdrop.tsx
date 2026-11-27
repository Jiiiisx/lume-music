import React from 'react';
import { StyleSheet, View } from 'react-native';
import { LinearGradient } from 'expo-linear-gradient';
import { usePlayer } from '../context/PlayerContext';

export function AmbientBackdrop({ children }: { children: React.ReactNode }) {
  const { palette } = usePlayer();

  return (
    <View style={styles.root}>
      <LinearGradient
        colors={[palette.bg_top, palette.bg_mid, palette.bg_bottom]}
        locations={[0, 0.42, 1]}
        style={StyleSheet.absoluteFill}
      />
      {children}
    </View>
  );
}

const styles = StyleSheet.create({
  root: {
    flex: 1,
  },
});
