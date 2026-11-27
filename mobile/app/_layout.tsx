import React from 'react';
import { Stack } from 'expo-router';
import { StatusBar } from 'expo-status-bar';
import { GestureHandlerRootView } from 'react-native-gesture-handler';
import { SafeAreaProvider } from 'react-native-safe-area-context';
import { TrackActionSheet } from '../src/components/TrackActionSheet';
import { PlayerProvider } from '../src/context/PlayerContext';
import { LumeTheme } from '../src/theme';

export default function RootLayout() {
  return (
    <GestureHandlerRootView style={{ flex: 1, backgroundColor: LumeTheme.bg }}>
      <SafeAreaProvider>
        <PlayerProvider>
          <StatusBar style="light" />
          <Stack
            screenOptions={{
              headerShown: false,
              contentStyle: { backgroundColor: LumeTheme.bg },
            }}
          >
            <Stack.Screen name="(tabs)" options={{ headerShown: false }} />
            <Stack.Screen
              name="player"
              options={{
                presentation: 'modal',
                animation: 'slide_from_bottom',
              }}
            />
            <Stack.Screen
              name="driving"
              options={{
                presentation: 'fullScreenModal',
                animation: 'fade',
              }}
            />
            <Stack.Screen
              name="queue"
              options={{
                presentation: 'modal',
                animation: 'slide_from_bottom',
              }}
            />
            <Stack.Screen
              name="settings"
              options={{
                presentation: 'modal',
                animation: 'slide_from_bottom',
              }}
            />
            <Stack.Screen name="artist/[id]" />
            <Stack.Screen name="album/[id]" />
            <Stack.Screen name="remote-playlist/[id]" />
          </Stack>
          <TrackActionSheet />
        </PlayerProvider>
      </SafeAreaProvider>
    </GestureHandlerRootView>
  );
}
